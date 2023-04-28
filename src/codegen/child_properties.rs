use std::io::{Result, Write};

use super::{
    general::{doc_alias, doc_hidden},
    property_body,
};
use crate::{
    analysis::{child_properties::ChildProperty, rust_type::RustType},
    chunk::Chunk,
    env::Env,
    library,
    nameutil::use_gtk_type,
    traits::IntoString,
    writer::{primitives::tabs, ToCode},
};

pub fn generate(
    w: &mut dyn Write,
    env: &Env,
    prop: &ChildProperty,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
) -> Result<()> {
    generate_func(w, env, prop, in_trait, only_declaration, indent, true)?;
    generate_func(w, env, prop, in_trait, only_declaration, indent, false)?;

    Ok(())
}

fn generate_func(
    w: &mut dyn Write,
    env: &Env,
    prop: &ChildProperty,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
    is_get: bool,
) -> Result<()> {
    let pub_prefix = if in_trait { "" } else { "pub " };
    let decl_suffix = if only_declaration { ";" } else { " {" };
    let type_string = RustType::try_new(env, prop.typ);
    let comment_prefix = if type_string.is_err() { "//" } else { "" };

    writeln!(w)?;

    doc_hidden(w, prop.doc_hidden, comment_prefix, indent)?;
    let decl = declaration(env, prop, is_get);
    let add_doc_alias = if is_get {
        prop.name != prop.getter_name && prop.name != prop.prop_name
    } else {
        prop.name != prop.prop_name
    };
    if add_doc_alias {
        doc_alias(
            w,
            &format!("{}.{}", &prop.child_name, &prop.name),
            comment_prefix,
            indent,
        )?;
    }
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
    let func_name = if is_get {
        format!("{}_{}", prop.child_name, prop.getter_name)
    } else {
        format!("set_{}_{}", prop.child_name, prop.prop_name)
    };
    let mut bounds = if let Some(typ) = prop.child_type {
        let child_type = RustType::try_new(env, typ).into_string();
        format!("T: IsA<{child_type}>")
    } else {
        format!("T: IsA<{}>", use_gtk_type(env, "Widget"))
    };
    if !is_get && !prop.bounds.is_empty() {
        bounds = format!("{}, {}", prop.bounds, bounds);
    }
    let return_str = if is_get {
        let dir = library::ParameterDirection::Return;
        let ret_type = RustType::builder(env, prop.typ)
            .direction(dir)
            .nullable(prop.nullable)
            .ref_mode(prop.get_out_ref_mode)
            .try_build_param()
            .into_string();
        format!(" -> {ret_type}")
    } else {
        String::new()
    };
    format!(
        "fn {}<{}>(&self, item: &T{}){}",
        func_name,
        bounds,
        if is_get {
            String::new()
        } else {
            format!(", {}", prop.set_params)
        },
        return_str
    )
}

fn body(env: &Env, prop: &ChildProperty, in_trait: bool, is_get: bool) -> Chunk {
    let mut builder = property_body::Builder::new_for_child_property(env);
    builder
        .name(&prop.name)
        .in_trait(in_trait)
        .var_name(&prop.prop_name)
        .is_get(is_get);

    if let Ok(type_) = RustType::try_new(env, prop.typ) {
        builder.type_(type_.as_str());
    } else {
        builder.type_("/*Unknown type*/");
    }

    builder.generate()
}
