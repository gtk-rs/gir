use std::io::{Result, Write};

use analysis::rust_type::AsStr;
use env::Env;
use library;
use super::ffi_type::*;
use super::super::general;

pub fn generate_classes_funcs<W: Write>(w: &mut W, env: &Env, classes: &Vec<&library::Class>) -> Result<()> {
    for klass in classes {
        try!(generate_class_funcs(w, env, klass));
    }

    Ok(())
}

fn generate_class_funcs<W: Write>(w: &mut W, env: &Env, klass: &library::Class) -> Result<()> {
    try!(writeln!(w, ""));
    try!(writeln!(w, "    //========================================================================="));
    try!(writeln!(w, "    // {}", klass.glib_type_name));
    try!(writeln!(w, "    //========================================================================="));
    try!(writeln!(w, "    pub fn {:<36}() -> GType;", klass.glib_get_type));

    for func in &klass.functions {
        let decl = function_declaration(env, func);
        try!(writeln!(w, "    {}", decl));
    }

    Ok(())
}

fn function_declaration(env: &Env, func: &library::Function) -> String {
    let (mut commented, ret_str) = function_return_value(env, func);

    let mut parameter_strs: Vec<String> = Vec::new();
    for par in &func.parameters {
        let (c, par_str) = function_parameter(env, par);
        parameter_strs.push(par_str);
        if c { commented = true; }
    }

    let commented_str = if commented { "//" } else { "" };
    format!("{}pub fn {:<36}({}){};", commented_str, func.c_identifier,
        parameter_strs.connect(", "), ret_str)
}

fn function_return_value(env: &Env, func: &library::Function) -> (bool, String) {
    if func.ret.typ == Default::default() { return (false, String::new()) }
    let ffi_type = ffi_type(env, func.ret.typ, &func.ret.c_type);
    let commented = ffi_type.is_err();
    (commented, format!(" -> {}", ffi_type.as_str()))
}

fn function_parameter(env: &Env, par: &library::Parameter) -> (bool, String) {
    if let &library::Type::Fundamental(library::Fundamental::VarArgs) = env.library.type_(par.typ) {
        return (false, "...".into());
    }
    let ffi_type = ffi_type(env, par.typ, &par.c_type);
    let commented = ffi_type.is_err();
    (commented, format!("{}: {}", general::fix_parameter_name(&par.name), ffi_type.as_str()))
}
