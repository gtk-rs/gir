use std::io::{Result, Write};

use analysis::bounds::Bounds;
use analysis::properties::Property;
use analysis::rust_type::{parameter_rust_type,rust_type};
use chunk::Chunk;
use env::Env;
use super::general::version_condition;
use library;
use writer::primitives::tabs;
use super::property_body;
use traits::IntoString;
use writer::ToCode;

pub fn generate(w: &mut Write, env: &Env, prop: &Property, in_trait: bool,
                only_declaration: bool, indent: usize) -> Result<()> {
    try!(generate_prop_func(w, env, prop, in_trait, only_declaration, indent));

    Ok(())
}

fn generate_prop_func(w: &mut Write, env: &Env, prop: &Property, in_trait: bool,
                     only_declaration: bool, indent: usize) -> Result<()> {
    let pub_prefix = if in_trait { "" } else { "pub " };
    let decl_suffix = if only_declaration { ";" } else { " {" };
    let type_string = rust_type(env, prop.typ);
    let commented = type_string.is_err() || (prop.default_value.is_none() && prop.is_get);

    let comment_prefix = if commented {
        "//"
    } else {
        ""
    };

    try!(writeln!(w, ""));

    let decl = declaration(env, prop);
    try!(version_condition(w, env, prop.version, commented, indent));
    try!(writeln!(w, "{}{}{}{}{}", tabs(indent),
        comment_prefix, pub_prefix, decl, decl_suffix));

    if !only_declaration {
        let body = body(prop).to_code(env);
        for s in body {
            try!(writeln!(w, "{}{}{}", tabs(indent), comment_prefix, s));
        }
    }

    Ok(())
}

fn declaration(env: &Env, prop: &Property) -> String {
    let mut bound = String::new();
    let set_param = if prop.is_get {
        "".to_string()
    } else {
        let dir = library::ParameterDirection::In;
        let param_type = match prop.bounds.get_parameter_alias_info(&prop.var_name) {
            Some((t, _)) => {
                bound = bounds(&prop.bounds);
                if *prop.nullable {
                    format!("Option<&{}>", t)
                } else {
                    format!("&{}", t)
                }
            }
            None => parameter_rust_type(env, prop.typ, dir, prop.nullable, prop.set_in_ref_mode)
                .into_string()
        };
        format!(", {}: {}", prop.var_name, param_type)
    };
    let return_str = if prop.is_get {
        let dir = library::ParameterDirection::Return;
        let ret_type = parameter_rust_type(env, prop.typ, dir, prop.nullable, prop.get_out_ref_mode)
            .into_string();
        format!(" -> {}", ret_type)
    } else {
        "".to_string()
    };
    format!("fn {}{}(&self{}){}", prop.func_name, bound, set_param, return_str)
}

pub fn bounds(bounds: &Bounds) -> String {
    use analysis::bounds::BoundType::*;
    if bounds.is_empty() { return String::new() }
    let strs: Vec<String> = bounds.iter()
        .map(|bound| match bound.bound_type {
            IsA => format!("{}: IsA<{}> + IsA<Object>", bound.alias, bound.type_str),
            _ => unreachable!(),
        })
        .collect();
    format!("<{}>", strs.join(", "))
}

fn body(prop: &Property) -> Chunk {
    let mut builder = property_body::Builder::new();
    builder.name(&prop.name)
        .var_name(&prop.var_name)
        .is_get(prop.is_get)
        .is_ref(prop.set_in_ref_mode.is_ref())
        .is_nullable(*prop.nullable)
        .conversion(prop.conversion);
    if let Some(ref default_value) = prop.default_value {
        builder.default_value(default_value);
    } else {
        builder.default_value("/*Unknown default value*/");
    }

    builder.generate()
}
