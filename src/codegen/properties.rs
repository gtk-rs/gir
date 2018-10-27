use std::io::{Result, Write};

use analysis;
use analysis::bounds::{Bound, Bounds};
use analysis::properties::Property;
use analysis::rust_type::{parameter_rust_type, rust_type};
use chunk::Chunk;
use codegen;
use env::Env;
use super::general::{cfg_deprecated, version_condition};
use library;
use writer::primitives::tabs;
use super::property_body;
use traits::IntoString;
use writer::ToCode;

pub fn generate(
    w: &mut Write,
    env: &Env,
    prop: &Property,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
) -> Result<()> {
    try!(generate_prop_func(
        w,
        env,
        prop,
        in_trait,
        only_declaration,
        indent,
    ));

    Ok(())
}

fn generate_prop_func(
    w: &mut Write,
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

    try!(writeln!(w));

    let decl = declaration(env, prop);
    if !in_trait || only_declaration {
        try!(cfg_deprecated(w, env, prop.deprecated_version, commented, indent));
    }
    try!(version_condition(w, env, prop.version, commented, indent));
    try!(writeln!(
        w,
        "{}{}{}{}{}",
        tabs(indent),
        comment_prefix,
        pub_prefix,
        decl,
        decl_suffix
    ));

    if !only_declaration {
        let body = body(env, prop).to_code(env);
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
        let param_type = if let Some(Bound {
            alias,
            ref type_str,
            ref bound_type,
            ..
        }) = prop.bound
        {
            use library::Type::*;
            use analysis::bounds::BoundType;

            let type_ = env.library.type_(prop.typ);
            let bound_type = match *type_ {
                Fundamental(_) => Some(bound_type.clone()),
                _ => match bound_type {
                    BoundType::Into(_, Some(ref x)) => Some(*x.clone()),
                    _ => None,
                }
            };
            match bound_type {
                Some(ref bound_type) if bound_type.is_into() => {
                    let type_name = parameter_rust_type(env,
                                                        prop.typ,
                                                        dir,
                                                        library::Nullable(false),
                                                        analysis::ref_mode::RefMode::ByRefFake)
                                        .into_string();
                    let mut bounds = Bounds::default();
                    bounds.add_parameter(&prop.var_name,
                                         &type_name,
                                         bound_type.clone(),
                                         false);
                    bound = codegen::function::bounds(&bounds, &[], false);
                    format!("{}", bounds.iter().next().unwrap().alias)
                }
                Some(_) => {
                    let value_bound = if !prop.is_get {
                        if *prop.nullable {
                            " + glib::value::SetValueOptional"
                        } else {
                            " + glib::value::SetValue"
                        }
                    } else {
                        ""
                    };
                    bound = format!(
                        "<{}: IsA<{}> + IsA<glib::object::Object>{}>",
                        alias,
                        type_str,
                        value_bound
                    );
                    if *prop.nullable {
                        format!("Option<&{}>", alias)
                    } else {
                        format!("&{}", alias)
                    }
                }
                _ => {
                    parameter_rust_type(env, prop.typ, dir, prop.nullable, prop.set_in_ref_mode)
                        .into_string()
                }
            }
        } else {
            parameter_rust_type(env, prop.typ, dir, prop.nullable, prop.set_in_ref_mode)
                .into_string()
        };
        format!(", {}: {}", prop.var_name, param_type)
    };
    let return_str = if prop.is_get {
        let dir = library::ParameterDirection::Return;
        let ret_type =
            parameter_rust_type(env, prop.typ, dir, prop.nullable, prop.get_out_ref_mode)
                .into_string();
        format!(" -> {}", ret_type)
    } else {
        "".to_string()
    };
    format!(
        "fn {}{}(&self{}){}",
        prop.func_name,
        bound,
        set_param,
        return_str
    )
}

fn body(env: &Env, prop: &Property) -> Chunk {
    let mut builder = property_body::Builder::new();
    builder
        .name(&prop.name)
        .var_name(&prop.var_name)
        .is_get(prop.is_get)
        .is_ref(prop.set_in_ref_mode.is_ref())
        .is_into({
            use library::Type::*;

            let type_ = env.library.type_(prop.typ);
            match *type_ {
                Fundamental(_) => {
                    if let Some(Bound { ref bound_type, .. }) = prop.bound {
                        bound_type.is_into()
                    } else {
                        false
                    }
                }
                _ => false,
            }
        })
        .is_nullable(*prop.nullable);

    if let Ok(type_) = rust_type(env, prop.typ) {
        builder.type_(&type_);
    } else {
        builder.type_("/*Unknown type*/");
    }

    builder.generate()
}
