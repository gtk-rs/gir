use std::io::{Result, Write};
use analysis::functions::Info;
use analysis::special_functions::{Infos, Type};

pub fn generate(w: &mut Write, type_name: &str, functions: &[Info], specials: &Infos, in_trait: bool)
        -> Result<()> {
    for (type_, name) in specials.iter() {
        match *type_ {
            Type::Compare => {
                if specials.get(&Type::Equal).is_none() {
                    try!(generate_eq_compare(w, type_name, lookup(functions, name), in_trait));
                }
                try!(generate_ord(w, type_name, lookup(functions, name), in_trait));
            }
            Type::Equal => {
                try!(generate_eq(w, type_name, lookup(functions, name), in_trait));
            }
            Type::ToString => try!(generate_display(w, type_name, lookup(functions, name), in_trait)),
            _ => {}
        }
    }
    Ok(())
}

fn lookup<'a>(functions: &'a [Info], name: &str) -> &'a Info {
    functions.iter()
        .find(|f| f.glib_name == name)
        .unwrap()
}

fn generate_display(w: &mut Write, type_name: &str, func: &Info, in_trait: bool) -> Result<()> {
    use analysis::out_parameters::Mode;

    let call = if in_trait {
        format!("{type_name}Ext::{func_name}(self)",
                type_name = type_name, func_name = func.name)
    } else {
        format!("self.{func_name}()",
                func_name = func.name)
    };

    let body = if let Mode::Throws(_) = func.outs.mode {
        format!("if let Ok(val) = {} {{
            write!(f, \"{{}}\", val)
        }} else {{
            Err(fmt::Error)
        }}", call)
    } else {
        format!("write!(f, \"{{}}\", {})", call)
    };
    writeln!(w, "
impl fmt::Display for {type_name} {{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {{
        {body}
    }}
}}", type_name = type_name, body = body)
}

fn generate_eq(w: &mut Write, type_name: &str, func: &Info, in_trait: bool) -> Result<()> {
    let call = if in_trait {
        format!("{type_name}Ext::{func_name}(self, other)",
                type_name = type_name, func_name = func.name)
    } else {
        format!("self.{func_name}(other)",
                func_name = func.name)
    };

    writeln!(w, "
impl PartialEq for {type_name} {{
    #[inline]
    fn eq(&self, other: &Self) -> bool {{
        {call}
    }}
}}

impl Eq for {type_name} {{}}", type_name = type_name, call = call)
}

fn generate_eq_compare(w: &mut Write, type_name: &str, func: &Info, in_trait: bool) -> Result<()> {
    let call = if in_trait {
        format!("{type_name}Ext::{func_name}(self, other)",
                type_name = type_name, func_name = func.name)
    } else {
        format!("self.{func_name}(other)",
                func_name = func.name)
    };

    writeln!(w, "
impl PartialEq for {type_name} {{
    #[inline]
    fn eq(&self, other: &Self) -> bool {{
        {call} == 0
    }}
}}

impl Eq for {type_name} {{}}", type_name = type_name, call = call)
}

fn generate_ord(w: &mut Write, type_name: &str, func: &Info, in_trait: bool) -> Result<()> {
    let call = if in_trait {
        format!("{type_name}Ext::{func_name}(self, other)",
                type_name = type_name, func_name = func.name)
    } else {
        format!("self.{func_name}(other)",
                func_name = func.name)
    };

    writeln!(w, "
impl PartialOrd for {type_name} {{
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {{
        {call}.partial_cmp(&0)
    }}
}}

impl Ord for {type_name} {{
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {{
        {call}.cmp(&0)
    }}
}}", type_name = type_name, call = call)
}
