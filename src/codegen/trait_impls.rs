use std::io::{Result, Write};
use analysis::functions::Info;
use analysis::special_functions::{Infos, Type};

pub fn generate(w: &mut Write, type_name: &str, functions: &[Info], specials: &Infos)
        -> Result<()> {
    for (type_, name) in specials.iter() {
        match *type_ {
            Type::Compare => {
                if specials.get(&Type::Equal).is_none() {
                    try!(generate_eq_compare(w, type_name, lookup(functions, name)));
                }
                try!(generate_ord(w, type_name, lookup(functions, name)));
            }
            Type::Equal => {
                try!(generate_eq(w, type_name, lookup(functions, name)));
            }
            Type::ToString => try!(generate_display(w, type_name, lookup(functions, name))),
            _ => {}
        }
    }
    Ok(())
}

fn lookup<'a>(functions: &'a [Info], name: &str) -> &'a Info {
    functions.iter()
        .filter(|f| f.glib_name == name)
        .next()
        .unwrap()
}

fn generate_display(w: &mut Write, type_name: &str, func: &Info) -> Result<()> {
    writeln!(w, "
impl fmt::Display for {type_name} {{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {{
        write!(f, \"{{}}\", self.{func_name}())
    }}
}}", type_name = type_name, func_name = func.name)
}

fn generate_eq(w: &mut Write, type_name: &str, func: &Info) -> Result<()> {
    writeln!(w, "
impl PartialEq for {type_name} {{
    #[inline]
    fn eq(&self, other: &Self) -> bool {{
        self.{func_name}(other)
    }}
}}

impl Eq for {type_name} {{}}", type_name = type_name, func_name = func.name)
}

fn generate_eq_compare(w: &mut Write, type_name: &str, func: &Info) -> Result<()> {
    writeln!(w, "
impl PartialEq for {type_name} {{
    #[inline]
    fn eq(&self, other: &Self) -> bool {{
        self.{func_name}(other) == 0
    }}
}}

impl Eq for {type_name} {{}}", type_name = type_name, func_name = func.name)
}

fn generate_ord(w: &mut Write, type_name: &str, func: &Info) -> Result<()> {
    writeln!(w, "
impl PartialOrd for {type_name} {{
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {{
        self.{func_name}(other).partial_cmp(&0)
    }}
}}

impl Ord for {type_name} {{
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {{
        self.{func_name}(other).cmp(&0)
    }}
}}", type_name = type_name, func_name = func.name)
}
