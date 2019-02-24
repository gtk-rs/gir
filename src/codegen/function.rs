use std::io::{Result, Write};

use library;
use analysis;
use analysis::bounds::{Bound, Bounds};
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
        suffix,
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
        analysis.ret.to_return_value(env, false)
    };
    let mut param_str = String::with_capacity(100);

    let (bounds, _) = bounds(&analysis.bounds, &[], false, false);

    for par in analysis.parameters.rust_parameters.iter() {
        if !param_str.is_empty() {
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
        return_str,
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
                if let Some(p) = analysis.bounds.get_base_alias(t) {
                    skipped_bounds.push(p);
                }
            }
            continue;
        }

        if pos - skipped > 0 {
            param_str.push_str(", ")
        }

        let s = c_par.to_parameter(env, &analysis.bounds);
        param_str.push_str(&s);
    }

    let (bounds, _) = bounds(&analysis.bounds, skipped_bounds.as_ref(), true, false);

    let where_str = if async_future.is_method {
        " where Self: Sized + Clone"
    } else {
        ""
    };

    format!(
        "fn {}{}({}){}{}",
        async_future.name,
        bounds,
        param_str,
        return_str,
        where_str,
    )
}

pub fn bound_to_string(bound: &Bound, async: bool) -> String {
    use analysis::bounds::BoundType::*;

    match bound.bound_type {
        NoWrapper => {
            format!("{}: {}", bound.alias, bound.type_str)
        }
        IsA(Some(lifetime)) => {
            format!("{}: IsA<{}> + {}", bound.alias, bound.type_str, if async { "Clone + 'static".into() } else { format!("'{}", lifetime) })
        }
        IsA(None) => format!("{}: IsA<{}>{}", bound.alias, bound.type_str, if async { " + Clone + 'static" } else { "" }),
        // This case should normally never happened
        AsRef(Some(_/*lifetime*/)) => {
            unreachable!();
            // format!("{}: AsRef<{}> + '{}", bound.alias, bound.type_str, lifetime)
        }
        AsRef(None) => format!("{}: AsRef<{}>", bound.alias, bound.type_str),
    }
}

pub fn bounds(
    bounds: &Bounds,
    skip: &[char],
    async: bool,
    filter_callback_modified: bool,
) -> (String, Vec<String>) {
    use analysis::bounds::BoundType::*;

    if bounds.is_empty() {
        return (String::new(), Vec::new());
    }

    let skip_lifetimes = bounds.iter()
        .filter(|bound| skip.contains(&bound.alias))
        .filter_map(|bound| match bound.bound_type {
            IsA(Some(lifetime)) |
            AsRef(Some(lifetime)) => Some(lifetime),
            _ => None,
        })
        .collect::<Vec<_>>();

    let strs: Vec<String> = bounds
        .iter_lifetimes()
        .filter(|s| !skip_lifetimes.contains(s))
        .map(|s| format!("'{}", s))
        .chain(bounds.iter()
                     .filter(|bound| !skip.contains(&bound.alias) && (!filter_callback_modified ||
                                                                      !bound.callback_modified))
                     .map(|b| bound_to_string(b, async)))
        .collect();

    if strs.is_empty() {
        (String::new(), Vec::new())
    } else {
        let bounds = bounds.iter_lifetimes()
                           .filter(|s| !skip_lifetimes.contains(s))
                           .map(|s| format!("'{}", s))
                           .chain(bounds.iter()
                                        .filter(|bound| !skip.contains(&bound.alias) &&
                                                        (!filter_callback_modified ||
                                                         !bound.callback_modified))
                                        .map(|b| b.alias.to_string()))
                           .collect::<Vec<_>>();
        (format!("<{}>", strs.join(", ")), bounds)
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
    } else {
        for trampoline in analysis.callbacks.iter() {
            builder.callback(trampoline);
        }
        for trampoline in analysis.destroys.iter() {
            builder.destroy(trampoline);
        }
    }

    for par in &analysis.parameters.c_parameters {
        if outs_as_return && analysis.outs.iter().any(|p| p.name == par.name) {
            builder.out_parameter(env, par);
        } else {
            builder.parameter();
        }
    }

    let (bounds, bounds_names) = bounds(&analysis.bounds, &[], false, true);

    builder.generate(env, bounds, bounds_names.join(", "))
}

pub fn body_chunk_futures(env: &Env, analysis: &analysis::functions::Info) -> StdResult<String, fmt::Error> {
    use std::fmt::Write;
    use analysis::ref_mode::RefMode;

    let async_future = analysis.async_future.as_ref().unwrap();

    let mut body = String::new();

    if env.config.library_name != "Gio" {
        try!(writeln!(body, "use gio::GioFuture;"));
    } else {
        try!(writeln!(body, "use GioFuture;"));
    }
    try!(writeln!(body, "use fragile::Fragile;"));
    try!(writeln!(body));

    let skip = if async_future.is_method { 1 } else { 0 };

    // Skip the instance parameter
    for par in analysis.parameters.rust_parameters.iter().skip(skip) {
        if par.name == "cancellable" || par.name == "callback" {
            continue;
        }

        let c_par = &analysis.parameters.c_parameters[par.ind_c];

        let type_ = env.type_(par.typ);
        let is_str = if let library::Type::Fundamental(library::Fundamental::Utf8) = *type_ { true } else { false };

        if *c_par.nullable {
            try!(writeln!(body, "let {} = {}.map(ToOwned::to_owned);", par.name, par.name));
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
        try!(writeln!(body, "\tlet cancellable = gio::Cancellable::new();"));
    } else {
        try!(writeln!(body, "\tlet cancellable = Cancellable::new();"));
    }
    try!(writeln!(body, "\tlet send = Fragile::new(send);"));

    if async_future.is_method {
        try!(writeln!(body, "\tlet obj_clone = Fragile::new(obj.clone());"));
        try!(writeln!(body, "\tobj.{}(", analysis.name));
    } else if analysis.type_name.is_ok() {
        try!(writeln!(body, "\tSelf::{}(", analysis.name));
    } else {
        try!(writeln!(body, "\t{}(", analysis.name));
    }

    // Skip the instance parameter
    for par in analysis.parameters.rust_parameters.iter().skip(skip) {
        if par.name == "cancellable" {
            try!(writeln!(body, "\t\tSome(&cancellable),"));
        } else if par.name == "callback" {
            continue;
        } else {
            let c_par = &analysis.parameters.c_parameters[par.ind_c];

            if *c_par.nullable {
                try!(writeln!(body, "\t\t{}.as_ref().map(::std::borrow::Borrow::borrow),",
                              par.name));
            } else if c_par.ref_mode != RefMode::None {
                try!(writeln!(body, "\t\t&{},", par.name));
            } else {
                try!(writeln!(body, "\t\t{},", par.name));
            }
        }
    }

    try!(writeln!(body, "\t\tmove |res| {{"));
    if async_future.is_method {
        try!(writeln!(body, "\t\t\tlet obj = obj_clone.into_inner();"));
        try!(writeln!(body, "\t\t\tlet res = res.map(|v| (obj.clone(), v)).map_err(|v| (obj.clone(), v));"));
    }
    try!(writeln!(body, "\t\t\tlet _ = send.into_inner().send(res);"));
    try!(writeln!(body, "\t\t}},"));
    try!(writeln!(body, "\t);"));
    try!(writeln!(body));
    try!(writeln!(body, "\tcancellable"));
    try!(writeln!(body, "}})"));

    Ok(body)
}
