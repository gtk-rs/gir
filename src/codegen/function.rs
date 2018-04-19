use std::io::{Result, Write};

use analysis;
use analysis::bounds::Bounds;
use analysis::functions::Visibility;
use analysis::namespaces;
use chunk::{ffi_function_todo, Chunk};
use env::Env;
use super::function_body_chunk;
use super::general::{
    cfg_condition, cfg_deprecated, doc_hidden, not_version_condition, version_condition,
};
use super::parameter::ToParameter;
use super::return_value::{out_parameters_as_return, ToReturnValue};
use writer::primitives::tabs;
use writer::ToCode;

pub fn generate(
    w: &mut Write,
    env: &Env,
    analysis: &analysis::functions::Info,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
) -> Result<()> {
    if analysis.is_async_finish(env) {
        return Ok(());
    }

    let mut commented = false;
    let mut comment_prefix = "";
    let mut pub_prefix = if in_trait { "" } else { "pub " };

    match analysis.visibility {
        Visibility::Public => {}
        Visibility::Comment => {
            commented = true;
            comment_prefix = "//";
        }
        Visibility::Private => if in_trait {
            warn!(
                "Generating trait method for private function {}",
                analysis.glib_name
            );
        } else {
            pub_prefix = "";
        },
        Visibility::Hidden => return Ok(()),
    }
    let declaration = declaration(env, analysis);
    let suffix = if only_declaration { ";" } else { " {" };

    try!(writeln!(w));
    if !in_trait || only_declaration {
        try!(cfg_deprecated(w, env, analysis.deprecated_version, commented, indent));
    }
    try!(cfg_condition(w, &analysis.cfg_condition, commented, indent));
    try!(version_condition(
        w,
        env,
        analysis.version,
        commented,
        indent,
    ));
    try!(not_version_condition(
        w,
        analysis.not_version,
        commented,
        indent,
    ));
    try!(doc_hidden(w, analysis.doc_hidden, comment_prefix, indent));
    try!(writeln!(
        w,
        "{}{}{}{}{}",
        tabs(indent),
        comment_prefix,
        pub_prefix,
        declaration,
        suffix
    ));

    if !only_declaration {
        let body = body_chunk(env, analysis).to_code(env);
        for s in body {
            try!(writeln!(w, "{}{}", tabs(indent), s));
        }
    }

    Ok(())
}

pub fn declaration(env: &Env, analysis: &analysis::functions::Info) -> String {
    let outs_as_return = !analysis.outs.is_empty();
    let return_str = if outs_as_return {
        out_parameters_as_return(env, analysis)
    } else if analysis.ret.bool_return_is_error.is_some() {
        if env.namespaces.glib_ns_id == namespaces::MAIN {
            " -> Result<(), error::BoolError>".into()
        } else {
            " -> Result<(), glib::error::BoolError>".into()
        }
    } else {
        analysis.ret.to_return_value(env)
    };
    let mut param_str = String::with_capacity(100);

    let bounds = bounds(&analysis.bounds);

    for (pos, par) in analysis.parameters.rust_parameters.iter().enumerate() {
        if pos > 0 {
            param_str.push_str(", ")
        }
        let c_par = &analysis.parameters.c_parameters[par.ind_c];
        let s = c_par.to_parameter(env, &analysis.bounds);
        param_str.push_str(&s);
    }

    format!(
        "fn {}{}({}){}",
        analysis.name,
        bounds,
        param_str,
        return_str
    )
}

pub fn bounds(bounds: &Bounds) -> String {
    use analysis::bounds::BoundType::*;
    if bounds.is_empty() {
        return String::new();
    }
    let strs: Vec<String> = bounds
        .iter_lifetimes()
        .map(|s| format!("'{}", s))
        .chain(bounds.iter().map(|bound| match bound.bound_type {
            NoWrapper => {
                format!("{}: {}", bound.alias, bound.type_str)
            }
            IsA(Some(lifetime)) => {
                format!("{}: IsA<{}> + '{}", bound.alias, bound.type_str, lifetime)
            }
            IsA(None) => format!("{}: IsA<{}>", bound.alias, bound.type_str),
            // This case should normally never happened
            AsRef(Some(lifetime)) => {
                format!("{}: AsRef<{}> + '{}", bound.alias, bound.type_str, lifetime)
            }
            AsRef(None) => format!("{}: AsRef<{}>", bound.alias, bound.type_str),
            Into(Some(l), _) => {
                format!("{}: Into<Option<&'{} {}>>", bound.alias, l, bound.type_str)
            }
            Into(None, _) => format!("{}: Into<Option<{}>>", bound.alias, bound.type_str),
        }))
        .collect();
    format!("<{}>", strs.join(", "))
}

pub fn body_chunk(env: &Env, analysis: &analysis::functions::Info) -> Chunk {
    if analysis.visibility == Visibility::Comment {
        return ffi_function_todo(&analysis.glib_name);
    }

    let outs_as_return = !analysis.outs.is_empty();

    let mut builder = function_body_chunk::Builder::new();
    builder
        .glib_name(&analysis.glib_name)
        .assertion(analysis.assertion)
        .ret(&analysis.ret)
        .transformations(&analysis.parameters.transformations)
        .outs_mode(analysis.outs.mode);

    if analysis.async {
        if let Some(ref trampoline) = analysis.trampoline {
            builder.async_trampoline(trampoline);
        } else {
            warn!("Async function {} has no associated _finish function", analysis.name);
        }
    }

    for par in &analysis.parameters.c_parameters {
        if outs_as_return && analysis.outs.iter().any(|p| p.name == par.name) {
            builder.out_parameter(env, par);
        } else {
            builder.parameter();
        }
    }

    builder.generate(env)
}
