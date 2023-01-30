use std::io::{Result, Write};

use once_cell::sync::Lazy;

use super::ffi_type::*;
use crate::{
    codegen::general::{cfg_condition, version_condition},
    config::{functions::Function, gobjects::GObject},
    env::Env,
    library, nameutil,
    traits::*,
};

// used as glib:get-type in GLib-2.0.gir
const INTERN: &str = "intern";

static DEFAULT_OBJ: Lazy<GObject> = Lazy::new(Default::default);

pub fn generate_records_funcs(
    w: &mut dyn Write,
    env: &Env,
    records: &[&library::Record],
) -> Result<()> {
    let intern_str = INTERN.to_string();
    for record in records {
        let name = format!("{}.{}", env.config.library_name, record.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        let version = obj.version.or(record.version);
        let glib_get_type = record.glib_get_type.as_ref().unwrap_or(&intern_str);
        generate_object_funcs(
            w,
            env,
            obj,
            version,
            &record.c_type,
            glib_get_type,
            &record.functions,
        )?;
    }

    Ok(())
}

pub fn generate_classes_funcs(
    w: &mut dyn Write,
    env: &Env,
    classes: &[&library::Class],
) -> Result<()> {
    for klass in classes {
        let name = format!("{}.{}", env.config.library_name, klass.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        let version = obj.version.or(klass.version);
        generate_object_funcs(
            w,
            env,
            obj,
            version,
            &klass.c_type,
            &klass.glib_get_type,
            &klass.functions,
        )?;
    }

    Ok(())
}

pub fn generate_bitfields_funcs(
    w: &mut dyn Write,
    env: &Env,
    bitfields: &[&library::Bitfield],
) -> Result<()> {
    let intern_str = INTERN.to_string();
    for bitfield in bitfields {
        let name = format!("{}.{}", env.config.library_name, bitfield.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        let version = obj.version.or(bitfield.version);
        let glib_get_type = bitfield.glib_get_type.as_ref().unwrap_or(&intern_str);
        generate_object_funcs(
            w,
            env,
            obj,
            version,
            &bitfield.c_type,
            glib_get_type,
            &bitfield.functions,
        )?;
    }

    Ok(())
}

pub fn generate_enums_funcs(
    w: &mut dyn Write,
    env: &Env,
    enums: &[&library::Enumeration],
) -> Result<()> {
    let intern_str = INTERN.to_string();
    for en in enums {
        let name = format!("{}.{}", env.config.library_name, en.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        let version = obj.version.or(en.version);
        let glib_get_type = en.glib_get_type.as_ref().unwrap_or(&intern_str);
        generate_object_funcs(
            w,
            env,
            obj,
            version,
            &en.c_type,
            glib_get_type,
            &en.functions,
        )?;
    }

    Ok(())
}

pub fn generate_unions_funcs(
    w: &mut dyn Write,
    env: &Env,
    unions: &[&library::Union],
) -> Result<()> {
    let intern_str = INTERN.to_string();
    for union in unions {
        let c_type = match union.c_type {
            Some(ref x) => x,
            None => return Ok(()),
        };
        let name = format!("{}.{}", env.config.library_name, union.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        let glib_get_type = union.glib_get_type.as_ref().unwrap_or(&intern_str);
        generate_object_funcs(
            w,
            env,
            obj,
            obj.version,
            c_type,
            glib_get_type,
            &union.functions,
        )?;
    }

    Ok(())
}

pub fn generate_interfaces_funcs(
    w: &mut dyn Write,
    env: &Env,
    interfaces: &[&library::Interface],
) -> Result<()> {
    for interface in interfaces {
        let name = format!("{}.{}", env.config.library_name, interface.name);
        let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
        let version = obj.version.or(interface.version);
        generate_object_funcs(
            w,
            env,
            obj,
            version,
            &interface.c_type,
            &interface.glib_get_type,
            &interface.functions,
        )?;
    }

    Ok(())
}

pub fn generate_other_funcs(
    w: &mut dyn Write,
    env: &Env,
    functions: &[library::Function],
) -> Result<()> {
    let name = format!("{}.*", env.config.library_name);
    let obj = env.config.objects.get(&name).unwrap_or(&DEFAULT_OBJ);
    generate_object_funcs(w, env, obj, None, "Other functions", INTERN, functions)
}

fn generate_cfg_configure(
    w: &mut dyn Write,
    configured_functions: &[&Function],
    commented: bool,
) -> Result<()> {
    let cfg_condition_ = configured_functions
        .iter()
        .find_map(|f| f.cfg_condition.as_ref());
    cfg_condition(w, cfg_condition_, commented, 1)?;
    Ok(())
}

fn generate_object_funcs(
    w: &mut dyn Write,
    env: &Env,
    obj: &GObject,
    version: Option<crate::version::Version>,
    c_type: &str,
    glib_get_type: &str,
    functions: &[library::Function],
) -> Result<()> {
    let write_get_type = glib_get_type != INTERN;
    if write_get_type || !functions.is_empty() {
        writeln!(w)?;
        writeln!(
            w,
            "    //========================================================================="
        )?;
        writeln!(w, "    // {c_type}")?;
        writeln!(
            w,
            "    //========================================================================="
        )?;
    }
    if write_get_type {
        let configured_functions = obj.functions.matched("get_type");

        if configured_functions
            .iter()
            .all(|f| f.status.need_generate())
        {
            let version = std::iter::once(version)
                .chain(configured_functions.iter().map(|f| f.version))
                .max()
                .flatten();
            version_condition(w, env, None, version, false, 1)?;
            generate_cfg_configure(w, &configured_functions, false)?;
            writeln!(w, "    pub fn {glib_get_type}() -> GType;")?;
        }
    }

    for func in functions {
        let configured_functions = obj.functions.matched(&func.name);
        if !configured_functions
            .iter()
            .all(|f| f.status.need_generate())
        {
            continue;
        }

        let (commented, sig) = function_signature(env, func, false);
        let comment = if commented { "//" } else { "" };

        // If a version was configured for this function specifically then use that,
        // otherwise use the (fixed up!) version of the function, if any, otherwise
        // use the version of the type.
        let version = configured_functions
            .iter()
            .map(|f| f.version)
            .max()
            .flatten()
            .or(func.version)
            .or(version);

        version_condition(w, env, None, version, commented, 1)?;
        let name = func.c_identifier.as_ref().unwrap();
        generate_cfg_configure(w, &configured_functions, commented)?;
        writeln!(w, "    {comment}pub fn {name}{sig};")?;
    }

    Ok(())
}

pub fn generate_callbacks(
    w: &mut dyn Write,
    env: &Env,
    callbacks: &[&library::Function],
) -> Result<()> {
    if !callbacks.is_empty() {
        writeln!(w, "// Callbacks")?;
    }
    for func in callbacks {
        let (commented, sig) = function_signature(env, func, true);
        let comment = if commented { "//" } else { "" };
        writeln!(
            w,
            "{}pub type {} = Option<unsafe extern \"C\" fn{}>;",
            comment,
            func.c_identifier.as_ref().unwrap(),
            sig
        )?;
    }
    if !callbacks.is_empty() {
        writeln!(w)?;
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
    if let library::Type::Basic(library::Basic::VarArgs) = env.library.type_(par.typ) {
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
