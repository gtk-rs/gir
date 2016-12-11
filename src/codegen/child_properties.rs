use std::io::{Result, Write};

use analysis::child_properties::ChildProperty;
use analysis::rust_type::{parameter_rust_type,rust_type};
use chunk::Chunk;
use env::Env;
use library;
use writer::primitives::tabs;
use nameutil;
use super::child_property_body;
use traits::IntoString;
use writer::ToCode;

pub fn generate(w: &mut Write, env: &Env, prop: &ChildProperty, in_trait: bool,
                only_declaration: bool, indent: usize) -> Result<()> {
    try!(generate_func(w, env, prop, in_trait, only_declaration, indent, true));
    try!(generate_func(w, env, prop, in_trait, only_declaration, indent, false));

    Ok(())
}

fn generate_func(w: &mut Write, env: &Env, prop: &ChildProperty, in_trait: bool,
                     only_declaration: bool, indent: usize, is_get: bool) -> Result<()> {
    let pub_prefix = if in_trait { "" } else { "pub " };
    let decl_suffix = if only_declaration { ";" } else { " {" };
    let type_string = rust_type(env, prop.typ);
    let comment_prefix = if type_string.is_err() || (prop.default_value.is_none() && is_get) {
        "//"
    } else {
        ""
    };
    let type_string = type_string.into_string();

    try!(writeln!(w, ""));

    let decl = declaration(env, prop, is_get);
    try!(writeln!(w, "{}{}{}{}{}", tabs(indent),
        comment_prefix, pub_prefix, decl, decl_suffix));

    if !only_declaration {
        let body = body(prop, &type_string, is_get).to_code(env);
        for s in body {
            try!(writeln!(w, "{}{}{}", tabs(indent), comment_prefix, s));
        }
    }

    Ok(())
}

fn declaration(env: &Env, prop: &ChildProperty, is_get: bool) -> String {
    let get_set = if is_get { "get" } else { "set" };
    let prop_name = nameutil::signal_to_snake(&*prop.name);
    let set_param = if is_get {
        "".to_string()
    } else {
        let dir = library::ParameterDirection::In;
        let param_type = parameter_rust_type(env, prop.typ, dir, prop.nullable, prop.set_in_ref_mode)
            .into_string();
        format!(", {}: {}", prop_name, param_type)
    };
    let bounds = if let Some(typ) = prop.child_type {
        let child_type = rust_type(env, typ).into_string();
        format!("<T: IsA<{}> + IsA<Widget>>", child_type)
    } else {
        "<T: IsA<Widget>>".to_string()
    };
    let return_str = if is_get {
        let dir = library::ParameterDirection::Return;
        let ret_type = parameter_rust_type(env, prop.typ, dir, prop.nullable, prop.get_out_ref_mode)
            .into_string();
        format!(" -> {}", ret_type)
    } else {
        "".to_string()
    };
    format!("fn {}_{}_{}{}(&self, item: &T{}){}", get_set, prop.child_name, prop_name, bounds,
            set_param, return_str)
}

fn body(prop: &ChildProperty, type_string: &str, is_get:bool ) -> Chunk {
    let mut builder = child_property_body::Builder::new();
    let prop_name = nameutil::signal_to_snake(&*prop.name);
    builder.name(&prop.name)
        .rust_name(&prop_name)
        .is_get(is_get)
        .type_string(type_string)
        .is_ref(prop.set_in_ref_mode.is_ref())
        .is_nullable(*prop.nullable);
    if let Some(ref default_value) = prop.default_value {
        builder.default_value(&default_value);
    } else {
        builder.default_value("/*Unknown default value*/");
    }

    builder.generate()
}
