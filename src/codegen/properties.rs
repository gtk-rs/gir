use std::io::{Result, Write};

use super::{
    general::{cfg_deprecated, doc_alias, version_condition},
    property_body,
};
use crate::{
    analysis::{properties::Property, rust_type::RustType},
    chunk::Chunk,
    env::Env,
    library,
    traits::IntoString,
    writer::{primitives::tabs, ToCode},
};

pub fn generate(
    w: &mut dyn Write,
    env: &Env,
    prop: &Property,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
) -> Result<()> {
    generate_prop_func(w, env, prop, in_trait, only_declaration, indent)?;

    Ok(())
}

fn generate_prop_func(
    w: &mut dyn Write,
    env: &Env,
    prop: &Property,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
) -> Result<()> {
    let pub_prefix = if in_trait { "" } else { "pub " };
    let decl_suffix = if only_declaration { ";" } else { " {" };
    let commented = RustType::try_new(env, prop.typ).is_err();
    let comment_prefix = if commented { "//" } else { "" };

    writeln!(w)?;

    let decl = declaration(env, prop);
    cfg_deprecated(
        w,
        env,
        Some(prop.typ),
        prop.deprecated_version,
        commented,
        indent,
    )?;
    version_condition(w, env, None, prop.version, commented, indent)?;
    let add_doc_alias = if let Some(func_name_alias) = prop.func_name_alias.as_ref() {
        &prop.name != func_name_alias && prop.name != prop.var_name
    } else {
        prop.name != prop.var_name
    };
    if add_doc_alias {
        doc_alias(w, &prop.name, comment_prefix, indent)?;
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
        let body = body(env, prop, in_trait).to_code(env);
        for s in body {
            writeln!(w, "{}{}{}", tabs(indent), comment_prefix, s)?;
        }
    }

    Ok(())
}

fn declaration(env: &Env, prop: &Property) -> String {
    let bound: String;
    let set_param = if prop.is_get {
        bound = String::new();
        String::new()
    } else if let Some(ref set_bound) = prop.set_bound {
        bound = format!("<{}: IsA<{}>>", set_bound.alias, set_bound.type_str);
        format!(", {}: Option<&{}>", prop.var_name, set_bound.alias)
    } else {
        bound = String::new();
        let dir = library::ParameterDirection::In;
        let param_type = RustType::builder(env, prop.typ)
            .direction(dir)
            .nullable(prop.nullable)
            .ref_mode(prop.set_in_ref_mode)
            .try_build_param()
            .into_string();
        format!(", {}: {}", prop.var_name, param_type)
    };
    let return_str = if prop.is_get {
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
        "fn {}{}(&self{}){}",
        prop.func_name, bound, set_param, return_str
    )
}

fn body(env: &Env, prop: &Property, in_trait: bool) -> Chunk {
    let mut builder = property_body::Builder::new(env);
    builder
        .name(&prop.name)
        .in_trait(in_trait)
        .var_name(&prop.var_name)
        .is_get(prop.is_get);

    if let Ok(type_) = RustType::try_new(env, prop.typ) {
        builder.type_(type_.as_str());
    } else {
        builder.type_("/*Unknown type*/");
    }

    builder.generate()
}
