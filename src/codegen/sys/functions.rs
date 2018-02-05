use std::io::{Result, Write};

use codegen::general::version_condition;
use config::gobjects::GObject;
use env::Env;
use library;
use nameutil;
use super::ffi_type::*;
use traits::*;
use regex::Regex;

//used as glib:get-type in GLib-2.0.gir
const INTERN: &str = "intern";

lazy_static! {
    static ref DEFAULT_OBJ: GObject = Default::default();
}

pub fn generate_records_funcs(
    w: &mut Write,
    env: &Env,
    records: &[&library::Record],
) -> Result<()> {
    let intern_str = INTERN.to_string();
    for record in records {
        // Nested structs tend to generate a duplicate function name,
        // this catches the nested struct and ignores function gen
        let s_regex = Regex::new(r"^\w+_s\d+$").unwrap();
        if !s_regex.is_match(&record.name) {
            let name = format!("{}.{}", env.config.library_name, record.name);
            let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
            let glib_get_type = record.glib_get_type.as_ref().unwrap_or(&intern_str);
            try!(generate_object_funcs(
                w,
                env,
                obj,
                &record.c_type,
                glib_get_type,
                &record.functions,
            ));
        }
    }

    Ok(())
}

pub fn generate_classes_funcs(w: &mut Write, env: &Env, classes: &[&library::Class]) -> Result<()> {
    for klass in classes {
        let name = format!("{}.{}", env.config.library_name, klass.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        try!(generate_object_funcs(
            w,
            env,
            obj,
            &klass.c_type,
            &klass.glib_get_type,
            &klass.functions,
        ));
    }

    Ok(())
}

pub fn generate_bitfields_funcs(
    w: &mut Write,
    env: &Env,
    bitfields: &[&library::Bitfield],
) -> Result<()> {
    let intern_str = INTERN.to_string();
    for bitfield in bitfields {
        let name = format!("{}.{}", env.config.library_name, bitfield.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        let glib_get_type = bitfield.glib_get_type.as_ref().unwrap_or(&intern_str);
        try!(generate_object_funcs(
            w,
            env,
            obj,
            &bitfield.c_type,
            glib_get_type,
            &bitfield.functions,
        ));
    }

    Ok(())
}

pub fn generate_enums_funcs(
    w: &mut Write,
    env: &Env,
    enums: &[&library::Enumeration],
) -> Result<()> {
    let intern_str = INTERN.to_string();
    for en in enums {
        let name = format!("{}.{}", env.config.library_name, en.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        let glib_get_type = en.glib_get_type.as_ref().unwrap_or(&intern_str);
        try!(generate_object_funcs(
            w,
            env,
            obj,
            &en.c_type,
            glib_get_type,
            &en.functions,
        ));
    }

    Ok(())
}

pub fn generate_unions_funcs(w: &mut Write, env: &Env, unions: &[&library::Union]) -> Result<()> {
    let intern_str = INTERN.to_string();
    for union in unions {
        let c_type = match union.c_type {
            Some(ref x) => x,
            None => return Ok(()),
        };
        let name = format!("{}.{}", env.config.library_name, union.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        let glib_get_type = union.glib_get_type.as_ref().unwrap_or(&intern_str);
        try!(generate_object_funcs(
            w,
            env,
            obj,
            c_type,
            glib_get_type,
            &union.functions,
        ));
    }

    Ok(())
}

pub fn generate_interfaces_funcs(
    w: &mut Write,
    env: &Env,
    interfaces: &[&library::Interface],
) -> Result<()> {
    for interface in interfaces {
        let name = format!("{}.{}", env.config.library_name, interface.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        try!(generate_object_funcs(
            w,
            env,
            obj,
            &interface.c_type,
            &interface.glib_get_type,
            &interface.functions,
        ));
    }

    Ok(())
}

pub fn generate_other_funcs(
    w: &mut Write,
    env: &Env,
    functions: &[library::Function],
) -> Result<()> {
    let name = format!("{}.*", env.config.library_name);
    let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
    generate_object_funcs(w, env, obj, "Other functions", INTERN, functions)
}

fn generate_object_funcs(
    w: &mut Write,
    env: &Env,
    obj: &GObject,
    c_type: &str,
    glib_get_type: &str,
    functions: &[library::Function],
) -> Result<()> {
    let write_get_type = glib_get_type != INTERN;
    if write_get_type || !functions.is_empty() {
        try!(writeln!(w, ""));
        try!(writeln!(
            w,
            "    //========================================================================="
        ));
        try!(writeln!(w, "    // {}", c_type));
        try!(writeln!(
            w,
            "    //========================================================================="
        ));
    }
    if write_get_type {
        try!(writeln!(w, "    pub fn {}() -> GType;", glib_get_type));
    }

    for func in functions {
        let configured_functions = obj.functions.matched(&func.name);
        if configured_functions.iter().any(|f| f.ignore) {
            continue;
        }
        let is_windows_utf8 = configured_functions.iter().any(|f| f.is_windows_utf8);

        let (commented, sig) = function_signature(env, func, false);
        let comment = if commented { "//" } else { "" };
        try!(version_condition(w, env, func.version, commented, 1));
        let name = func.c_identifier.as_ref().unwrap();
        // since we work with gir-files from Linux, some function names need to be adjusted
        if is_windows_utf8 {
            try!(writeln!(
                w,
                "    {}#[cfg(any(windows, feature = \"dox\"))]",
                comment
            ));
            try!(writeln!(w, "    {}pub fn {}_utf8{};", comment, name, sig));
            try!(version_condition(w, env, func.version, commented, 1));
        }
        try!(writeln!(w, "    {}pub fn {}{};", comment, name, sig));
    }

    Ok(())
}

pub fn generate_callbacks(
    w: &mut Write,
    env: &Env,
    callbacks: &[&library::Function],
) -> Result<()> {
    if !callbacks.is_empty() {
        try!(writeln!(w, "// Callbacks"));
    }
    for func in callbacks {
        let (commented, sig) = function_signature(env, func, true);
        let comment = if commented { "//" } else { "" };
        try!(writeln!(
            w,
            "{}pub type {} = Option<unsafe extern \"C\" fn{}>;",
            comment,
            func.c_identifier.as_ref().unwrap(),
            sig
        ));
    }
    if !callbacks.is_empty() {
        try!(writeln!(w, ""));
    }

    Ok(())
}

pub fn function_signature(env: &Env, func: &library::Function, bare: bool) -> (bool, String) {
    let (mut commented, ret_str) = function_return_value(env, func);

    let mut parameter_strs: Vec<String> = Vec::new();
    for par in &func.parameters {
        let (c, par_str) = function_parameter(env, par, bare);
        parameter_strs.push(par_str);
        if c {
            commented = true;
        }
    }

    (
        commented,
        format!("({}){}", parameter_strs.join(", "), ret_str),
    )
}

fn function_return_value(env: &Env, func: &library::Function) -> (bool, String) {
    if func.ret.typ == Default::default() {
        return (false, String::new());
    }
    let ffi_type = ffi_type(env, func.ret.typ, &func.ret.c_type);
    let commented = ffi_type.is_err();
    (commented, format!(" -> {}", ffi_type.into_string()))
}

fn function_parameter(env: &Env, par: &library::Parameter, bare: bool) -> (bool, String) {
    if let library::Type::Fundamental(library::Fundamental::VarArgs) = *env.library.type_(par.typ) {
        return (false, "...".into());
    }
    let ffi_type = ffi_type(env, par.typ, &par.c_type);
    let commented = ffi_type.is_err();
    let res = if bare {
        ffi_type.into_string()
    } else {
        format!(
            "{}: {}",
            nameutil::mangle_keywords(&*par.name),
            ffi_type.into_string()
        )
    };
    (commented, res)
}
