use std::io::{Result, Write};

use env::Env;
use library;
use super::ffi_type::*;
use super::super::general;
use traits::*;

pub fn generate_classes_funcs<W: Write>(w: &mut W, env: &Env, classes: &[&library::Class]) -> Result<()> {
    for klass in classes {
        try!(generate_object_funcs(w, env, &klass.glib_type_name,
            &klass.glib_get_type, &klass.functions));
    }

    Ok(())
}

pub fn generate_interfaces_funcs<W: Write>(w: &mut W, env: &Env, interfaces: &[&library::Interface]) -> Result<()> {
    for interface in interfaces {
        try!(generate_object_funcs(w, env,  &interface.glib_type_name,
            &interface.glib_get_type, &interface.functions));
    }

    Ok(())
}

fn generate_object_funcs<W: Write>(w: &mut W, env: &Env, glib_type_name: &str,
    glib_get_type: &str, functions: &[library::Function]) -> Result<()> {
    try!(writeln!(w, ""));
    try!(writeln!(w, "    //========================================================================="));
    try!(writeln!(w, "    // {}", glib_type_name));
    try!(writeln!(w, "    //========================================================================="));
    try!(writeln!(w, "    pub fn {:<36}() -> GType;", glib_get_type));

    for func in functions {
        let (commented, sig) = function_signature(env, func, false);
        let comment = if commented { "//" } else { "" };
        try!(writeln!(w, "    {}pub fn {:<36}{};",
                      comment, func.c_identifier.as_ref().unwrap(), sig));
    }

    Ok(())
}

pub fn generate_callbacks<W: Write>(w: &mut W, env: &Env, callbacks: &[&library::Function]) -> Result<()> {
    for func in callbacks {
        let (commented, sig) = function_signature(env, func, true);
        let comment = if commented { "//" } else { "" };
        try!(writeln!(w, "{}pub type {} = unsafe extern \"C\" fn{};",
                      comment, func.c_identifier.as_ref().unwrap(), sig));
    }

    Ok(())
}

fn function_signature(env: &Env, func: &library::Function, bare: bool) -> (bool, String) {
    let (mut commented, ret_str) = function_return_value(env, func);

    let mut parameter_strs: Vec<String> = Vec::new();
    for par in &func.parameters {
        let (c, par_str) = function_parameter(env, par, bare);
        parameter_strs.push(par_str);
        if c { commented = true; }
    }

    (commented, format!("({}){}", parameter_strs.connect(", "), ret_str))
}

fn function_return_value(env: &Env, func: &library::Function) -> (bool, String) {
    if func.ret.typ == Default::default() { return (false, String::new()) }
    let ffi_type = ffi_type(env, func.ret.typ, &func.ret.c_type);
    let commented = ffi_type.is_err();
    (commented, format!(" -> {}", ffi_type.as_str()))
}

fn function_parameter(env: &Env, par: &library::Parameter, bare: bool) -> (bool, String) {
    if let &library::Type::Fundamental(library::Fundamental::VarArgs) = env.library.type_(par.typ) {
        return (false, "...".into());
    }
    let ffi_type = ffi_type(env, par.typ, &par.c_type);
    let commented = ffi_type.is_err();
    let res = if bare {
        ffi_type.as_str().into()
    }
    else {
        format!("{}: {}", general::fix_parameter_name(&par.name), ffi_type.as_str())
    };
    (commented, res)
}
