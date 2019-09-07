use super::{
    general::{cfg_deprecated, version_condition},
    property_body,
};
use crate::{
    analysis::{
        properties::Property,
        rust_type::{parameter_rust_type, rust_type},
    },
    chunk::Chunk,
    env::Env,
    library,
    traits::IntoString,
    writer::{primitives::tabs, ToCode},
};
use std::io::{Result, Write};

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
    let type_string = rust_type(env, prop.typ);
    let commented = type_string.is_err();

    let comment_prefix = if commented { "//" } else { "" };

    writeln!(w)?;

    let decl = declaration(env, prop);
    if !in_trait || only_declaration {
        cfg_deprecated(w, env, prop.deprecated_version, commented, indent)?;
    }
    version_condition(w, env, prop.version, commented, indent)?;
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
        bound = format!(
            "<{}: IsA<{}> + SetValueOptional>",
            set_bound.alias, set_bound.type_str
        );
        format!(", {}: Option<&{}>", prop.var_name, set_bound.alias)
    } else {
        bound = String::new();
        let dir = library::ParameterDirection::In;
        let param_type = parameter_rust_type(
            env,
            prop.typ,
            dir,
            prop.nullable,
            prop.set_in_ref_mode,
            library::ParameterScope::None,
        )
        .into_string();
        format!(", {}: {}", prop.var_name, param_type)
    };
    let return_str = if prop.is_get {
        let dir = library::ParameterDirection::Return;
        let ret_type = parameter_rust_type(
            env,
            prop.typ,
            dir,
            prop.nullable,
            prop.get_out_ref_mode,
            library::ParameterScope::None,
        )
        .into_string();
        format!(" -> {}", ret_type)
    } else {
        "".to_string()
    };
    format!(
        "fn {}{}(&self{}){}",
        prop.func_name, bound, set_param, return_str
    )
}

fn body(env: &Env, prop: &Property, in_trait: bool) -> Chunk {
    let mut builder = property_body::Builder::new();
    builder
        .name(&prop.name)
        .in_trait(in_trait)
        .var_name(&prop.var_name)
        .is_get(prop.is_get)
        .is_ref(prop.set_in_ref_mode.is_ref())
        .is_nullable(*prop.nullable);

    if let Ok(type_) = rust_type(env, prop.typ) {
        builder.type_(&type_);
    } else {
        builder.type_("/*Unknown type*/");
    }

    builder.generate()
}
