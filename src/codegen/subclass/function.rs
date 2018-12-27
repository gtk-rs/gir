use std::io::{Result, Write};

use library;
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

use std::result::Result as StdResult;
use std::fmt;

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

    if analysis.async_future.is_some() {
        let declaration = declaration_futures(env, analysis);
        let suffix = if only_declaration { ";" } else { " {" };

        try!(writeln!(w));
        if !in_trait || only_declaration {
            try!(cfg_deprecated(w, env, analysis.deprecated_version, commented, indent));
        }

        try!(writeln!(w, "{}{}#[cfg(feature = \"futures\")]", tabs(indent), comment_prefix));
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
            let body = body_chunk_futures(env, analysis).unwrap();
            for s in body.lines() {
                if !s.is_empty() {
                    try!(writeln!(w, "{}{}{}", tabs(indent+1), comment_prefix, s));
                } else {
                    try!(writeln!(w));
                }
            }
            try!(writeln!(w, "{}{}}}", tabs(indent), comment_prefix));
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

    let bounds = bounds(&analysis.bounds, &[], false);

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

pub fn declaration_futures(env: &Env, analysis: &analysis::functions::Info) -> String {
    let async_future = analysis.async_future.as_ref().unwrap();

    let return_str = if async_future.is_method {
        format!(" -> Box_<futures_core::Future<Item = (Self, {}), Error = (Self, {})>>", async_future.success_parameters, async_future.error_parameters)
    } else {
        format!(" -> Box_<futures_core::Future<Item = {}, Error = {}>>", async_future.success_parameters, async_future.error_parameters)
    };

    let mut param_str = String::with_capacity(100);

    let mut skipped = 0;
    let mut skipped_bounds = vec![];
    for (pos, par) in analysis.parameters.rust_parameters.iter().enumerate() {
        let c_par = &analysis.parameters.c_parameters[par.ind_c];

        if c_par.name == "callback" || c_par.name == "cancellable" {
            skipped += 1;
            if let Some((t, _)) = analysis.bounds.get_parameter_alias_info(&c_par.name) {
                skipped_bounds.push(t);
            }
            continue;
        }

        if pos - skipped > 0 {
            param_str.push_str(", ")
        }

        let s = c_par.to_parameter(env, &analysis.bounds);
        param_str.push_str(&s);
    }

    let bounds = bounds(&analysis.bounds, skipped_bounds.as_ref(), true);

    format!(
        "fn {}{}({}){}",
        async_future.name,
        bounds,
        param_str,
        return_str
    )
}

pub fn bounds(bounds: &Bounds, skip: &[char], async: bool) -> String {
    use analysis::bounds::BoundType::*;
    if bounds.is_empty() {
        return String::new();
    }

    let skip_lifetimes = bounds.iter()
        .filter(|bound| skip.contains(&bound.alias))
        .filter_map(|bound| match bound.bound_type {
            IsA(Some(lifetime)) |
            AsRef(Some(lifetime)) |
            Into(Some(lifetime), _) => Some(lifetime),
            _ => None,
        })
        .collect::<Vec<_>>();

    let strs: Vec<String> = bounds
        .iter_lifetimes()
        .filter(|s| !skip_lifetimes.contains(s))
        .map(|s| format!("'{}", s))
        .chain(bounds.iter().filter(|bound| !skip.contains(&bound.alias)).map(|bound| match bound.bound_type {
            NoWrapper => {
                format!("{}: {}", bound.alias, bound.type_str)
            }
            IsA(Some(lifetime)) => {
                format!("{}: IsA<{}> + {}", bound.alias, bound.type_str, if async { "Clone + 'static".into() } else { format!("'{}", lifetime) })
            }
            IsA(None) => format!("{}: IsA<{}>{}", bound.alias, bound.type_str, if async { " + Clone + 'static" } else { "" }),
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

    if strs.is_empty() {
        String::new()
    } else {
        format!("<{}>", strs.join(", "))
    }
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

pub fn body_chunk_futures(env: &Env, analysis: &analysis::functions::Info) -> StdResult<String, fmt::Error> {
    use std::fmt::Write;
    use analysis::bounds::BoundType;
    use analysis::ref_mode::RefMode;

    let async_future = analysis.async_future.as_ref().unwrap();

    let mut body = String::new();

    if env.config.library_name != "Gio" {
        try!(writeln!(body, "use gio::GioFuture;"));
    } else {
        try!(writeln!(body, "use GioFuture;"));
    }
    try!(writeln!(body, "use send_cell::SendCell;"));
    try!(writeln!(body));

    let skip = if async_future.is_method { 1 } else { 0 };

    // Skip the instance parameter
    for par in analysis.parameters.rust_parameters.iter().skip(skip) {
        if par.name == "cancellable" || par.name == "callback" {
            continue;
        }

        let c_par = &analysis.parameters.c_parameters[par.ind_c];

        let bounds = analysis.bounds.get_parameter_alias_info(&par.name);
        let is_into = if let Some((_, BoundType::Into(..))) = bounds { true } else { false };

        let type_ = env.type_(par.typ);
        let is_str = if let library::Type::Fundamental(library::Fundamental::Utf8) = *type_ { true } else { false };

        if is_into {
            try!(writeln!(body, "let {} = {}.into();", par.name, par.name));
            if is_str || c_par.nullable.0 {
                try!(writeln!(body, "let {} = {}.map(ToOwned::to_owned);", par.name, par.name));
            }
        } else if is_str {
            try!(writeln!(body, "let {} = String::from({});", par.name, par.name));
        } else if c_par.ref_mode != RefMode::None {
            try!(writeln!(body, "let {} = {}.clone();", par.name, par.name));
        }
    }

    if async_future.is_method {
        try!(writeln!(body, "GioFuture::new(self, move |obj, send| {{"));
    } else {
        try!(writeln!(body, "GioFuture::new(&(), move |_obj, send| {{"));
    }

    if env.config.library_name != "Gio" {
        try!(writeln!(body, "    let cancellable = gio::Cancellable::new();"));
    } else {
        try!(writeln!(body, "    let cancellable = Cancellable::new();"));
    }
    try!(writeln!(body, "    let send = SendCell::new(send);"));

    if async_future.is_method {
        try!(writeln!(body, "    let obj_clone = SendCell::new(obj.clone());"));
        try!(writeln!(body, "    obj.{}(", analysis.name));
    } else if analysis.type_name.is_ok() {
        try!(writeln!(body, "    Self::{}(", analysis.name));
    } else {
        try!(writeln!(body, "    {}(", analysis.name));
    }

    // Skip the instance parameter
    for par in analysis.parameters.rust_parameters.iter().skip(skip) {
        if par.name == "cancellable" {
            try!(writeln!(body, "         Some(&cancellable),"));
        } else if par.name == "callback" {
            continue;
        } else {
            let c_par = &analysis.parameters.c_parameters[par.ind_c];

            let bounds = analysis.bounds.get_parameter_alias_info(&par.name);
            let is_into = if let Some((_, BoundType::Into(..))) = bounds { true } else { false };

            if is_into {
                try!(writeln!(body, "         {}.as_ref().map(::std::borrow::Borrow::borrow),", par.name));
            } else if c_par.ref_mode != RefMode::None {
                try!(writeln!(body, "         &{},", par.name));
            } else {
                try!(writeln!(body, "         {},", par.name));
            }
        }
    }

    try!(writeln!(body, "         move |res| {{"));
    if async_future.is_method {
        try!(writeln!(body, "             let obj = obj_clone.into_inner();"));
        try!(writeln!(body, "             let res = res.map(|v| (obj.clone(), v)).map_err(|v| (obj.clone(), v));"));
    }
    try!(writeln!(body, "             let _ = send.into_inner().send(res);"));
    try!(writeln!(body, "         }},"));
    try!(writeln!(body, "    );"));
    try!(writeln!(body));
    try!(writeln!(body, "    cancellable"));
    try!(writeln!(body, "}})"));

    Ok(body)
}
