use std::io::{Result, Write};

use crate::analysis::child_properties::ChildProperty;
use crate::analysis::rust_type::{parameter_rust_type, rust_type};
use crate::chunk::Chunk;
use crate::env::Env;
use crate::library;
use crate::writer::primitives::tabs;
use crate::nameutil;
use super::general::doc_hidden;
use super::property_body;
use crate::traits::IntoString;
use crate::writer::ToCode;

pub fn generate(
    w: &mut Write,
    env: &Env,
    prop: &ChildProperty,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
) -> Result<()> {
    generate_func(
        w,
        env,
        prop,
        in_trait,
        only_declaration,
        indent,
        true,
    )?;
    generate_func(
        w,
        env,
        prop,
        in_trait,
        only_declaration,
        indent,
        false,
    )?;

    Ok(())
}

fn generate_func(
    w: &mut Write,
    env: &Env,
    prop: &ChildProperty,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
    is_get: bool,
) -> Result<()> {
    let pub_prefix = if in_trait { "" } else { "pub " };
    let decl_suffix = if only_declaration { ";" } else { " {" };
    let type_string = rust_type(env, prop.typ);
    let comment_prefix = if type_string.is_err() {
        "//"
    } else {
        ""
    };

    writeln!(w)?;

    doc_hidden(w, prop.doc_hidden, comment_prefix, indent)?;
    let decl = declaration(env, prop, is_get);
    writeln!(
        w,
        "{}{}{}{}{}",
        tabs(indent),
        comment_prefix,
        pub_prefix,
        decl,
        decl_suffix
    )?;

    if !only_declaration {
        let body = body(env, prop, in_trait, is_get).to_code(env);
        for s in body {
            writeln!(w, "{}{}{}", tabs(indent), comment_prefix, s)?;
        }
    }

    Ok(())
}

fn declaration(env: &Env, prop: &ChildProperty, is_get: bool) -> String {
    let get_set = if is_get { "get" } else { "set" };
    let prop_name = nameutil::signal_to_snake(&*prop.name);
    let func_name = format!("{}_{}_{}", get_set, prop.child_name, prop_name);
    let mut bounds = if let Some(typ) = prop.child_type {
        let child_type = rust_type(env, typ).into_string();
        format!("T: IsA<{}>", child_type)
    } else {
        "T: IsA<Widget>".to_string()
    };
    if !is_get && !prop.bounds.is_empty() {
        bounds = format!("{}, {}", prop.bounds, bounds);
    }
    let return_str = if is_get {
        let dir = library::ParameterDirection::Return;
        let ret_type =
            parameter_rust_type(env, prop.typ, dir, prop.nullable, prop.get_out_ref_mode,
                                library::ParameterScope::None)
                .into_string();
        format!(" -> {}", ret_type)
    } else {
        "".to_string()
    };
    format!(
        "fn {}<{}>(&self, item: &T{}){}",
        func_name,
        bounds,
        if is_get {
            "".to_owned()
        } else {
            format!(", {}", prop.set_params)
        },
        return_str
    )
}

fn body(env: &Env, prop: &ChildProperty, in_trait: bool, is_get: bool) -> Chunk {
    let mut builder = property_body::Builder::new_for_child_property();
    let prop_name = nameutil::signal_to_snake(&*prop.name);
    builder
        .name(&prop.name)
        .in_trait(in_trait)
        .var_name(&prop_name)
        .is_get(is_get)
        .is_ref(prop.set_in_ref_mode.is_ref())
        .is_nullable(*prop.nullable);

    if let Ok(type_) = rust_type(env, prop.typ) {
        builder.type_(&type_);
    } else {
        builder.type_("/*Unknown type*/");
    }

    builder.generate()
}
