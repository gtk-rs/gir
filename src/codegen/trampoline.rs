use std::io::{Result, Write};

use env::Env;
use analysis::bounds::{Bounds, BoundType};
use analysis::parameter::Parameter;
use analysis::ref_mode::RefMode;
use analysis::rust_type::parameter_rust_type;
use analysis::trampolines::Trampoline;
use super::return_value::ToReturnValue;
use traits::IntoString;

pub fn generate(w: &mut Write, env: &Env, analysis: &Trampoline,
                in_trait: bool, object_name: &str) -> Result<()> {
    try!(writeln!(w, ""));
    let (bounds, end) = if in_trait {
        ("<T>", "")
    } else {
        ("", " {")
    };

    let func_str = func_string(env, analysis);

    //TODO: version, cfg_condition
    try!(writeln!(w, "unsafe extern \"C\" fn {}{}(/*TODO: params*/, f: &Box<{}>){}",
                  analysis.name, bounds, func_str, end));
    if in_trait {
        try!(writeln!(w, "where T: IsA<{}> {{", object_name));
    }
    try!(writeln!(w, "\tcallback_guard!();"));
    try!(writeln!(w, "\t//TODO: body"));
    try!(writeln!(w, "}}"));

    Ok(())
}

fn func_string(env: &Env, analysis: &Trampoline) -> String {
    let param_str = func_parameters(env, analysis);
    let return_str = func_returns(env, analysis);

    format!("Fn({}){} + 'static", param_str, return_str)
}

fn func_parameters(env: &Env, analysis: &Trampoline) -> String {
    let mut param_str = String::with_capacity(100);

    for (pos, par) in analysis.parameters.iter().enumerate() {
        if pos > 0 { param_str.push_str(", ") }
        let s = func_parameter(env, par, &analysis.bounds);
        param_str.push_str(&s);
    }

    param_str
}

fn func_parameter(env: &Env, par: &Parameter, bounds: &Bounds) -> String {
    let mut_str = if par.ref_mode == RefMode::ByRefMut { "mut " } else { "" };

    let type_str: String;
    match bounds.get_parameter_alias_info(&par.name) {
        Some((t, bound_type)) => {
            match bound_type {
                BoundType::IsA => if *par.nullable {
                    type_str = format!("Option<&{}{}>", mut_str, t)
                } else {
                    type_str = format!("&{}{}", mut_str, t)
                },
                BoundType::AsRef  => type_str = t.to_owned(),
            }
        }
        None => {
            let rust_type = parameter_rust_type(env, par.typ, par.direction,
                                                par.nullable, par.ref_mode);
            type_str = rust_type.into_string();
        }
    }
    type_str
}

fn func_returns(env: &Env, analysis: &Trampoline) -> String {
    if analysis.ret.typ == Default::default() {
        String::new()
    } else {
        analysis.ret.to_return_value(&env)
    }
}
