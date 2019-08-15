use super::{
    function_body_chunk,
    general::{
        cfg_condition, cfg_deprecated, doc_hidden, not_version_condition, version_condition,
    },
    parameter::ToParameter,
    return_value::{out_parameters_as_return, ToReturnValue},
};
use crate::{
    analysis::{
        self,
        bounds::{Bound, Bounds},
        functions::Visibility,
        namespaces,
    },
    chunk::{ffi_function_todo, Chunk},
    env::Env,
    library,
    writer::{primitives::tabs, ToCode},
};
use log::warn;
use std::{
    fmt,
    io::{Result, Write},
    result::Result as StdResult,
};

pub fn generate(
    w: &mut dyn Write,
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
        Visibility::Private => {
            if in_trait {
                warn!(
                    "Generating trait method for private function {}",
                    analysis.glib_name
                );
            } else {
                pub_prefix = "";
            }
        }
        Visibility::Hidden => return Ok(()),
    }
    let declaration = declaration(env, analysis);
    let suffix = if only_declaration { ";" } else { " {" };

    writeln!(w)?;
    if !in_trait || only_declaration {
        cfg_deprecated(w, env, analysis.deprecated_version, commented, indent)?;
    }
    cfg_condition(w, &analysis.cfg_condition, commented, indent)?;
    version_condition(w, env, analysis.version, commented, indent)?;
    not_version_condition(w, analysis.not_version, commented, indent)?;
    doc_hidden(w, analysis.doc_hidden, comment_prefix, indent)?;
    writeln!(
        w,
        "{}{}{}{}{}",
        tabs(indent),
        comment_prefix,
        pub_prefix,
        declaration,
        suffix,
    )?;

    if !only_declaration {
        let body = body_chunk(env, analysis).to_code(env);
        for s in body {
            writeln!(w, "{}{}", tabs(indent), s)?;
        }
    }

    if analysis.async_future.is_some() {
        let declaration = declaration_futures(env, analysis);
        let suffix = if only_declaration { ";" } else { " {" };

        writeln!(w)?;
        if !in_trait || only_declaration {
            cfg_deprecated(w, env, analysis.deprecated_version, commented, indent)?;
        }

        writeln!(
            w,
            "{}{}#[cfg(any(feature = \"futures\", feature = \"dox\"))]",
            tabs(indent),
            comment_prefix
        )?;
        cfg_condition(w, &analysis.cfg_condition, commented, indent)?;
        version_condition(w, env, analysis.version, commented, indent)?;
        not_version_condition(w, analysis.not_version, commented, indent)?;
        doc_hidden(w, analysis.doc_hidden, comment_prefix, indent)?;
        writeln!(
            w,
            "{}{}{}{}{}",
            tabs(indent),
            comment_prefix,
            pub_prefix,
            declaration,
            suffix
        )?;

        if !only_declaration {
            let body = body_chunk_futures(env, analysis).unwrap();
            for s in body.lines() {
                if !s.is_empty() {
                    writeln!(w, "{}{}{}", tabs(indent + 1), comment_prefix, s)?;
                } else {
                    writeln!(w)?;
                }
            }
            writeln!(w, "{}{}}}", tabs(indent), comment_prefix)?;
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
        analysis.name, bounds, param_str, return_str,
    )
}

pub fn declaration_futures(env: &Env, analysis: &analysis::functions::Info) -> String {
    let async_future = analysis.async_future.as_ref().unwrap();

    let return_str = format!(
        " -> Box_<dyn future::Future<Output = Result<{}, {}>> + std::marker::Unpin>",
        async_future.success_parameters, async_future.error_parameters
    );

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

    format!(
        "fn {}{}({}){}",
        async_future.name, bounds, param_str, return_str,
    )
}

pub fn bound_to_string(bound: &Bound, r#async: bool) -> String {
    use crate::analysis::bounds::BoundType::*;

    match bound.bound_type {
        NoWrapper => format!("{}: {}", bound.alias, bound.type_str),
        IsA(Some(lifetime)) => format!(
            "{}: IsA<{}> + {}",
            bound.alias,
            bound.type_str,
            if r#async {
                "Clone + 'static".into()
            } else {
                format!("'{}", lifetime)
            }
        ),
        IsA(None) => format!(
            "{}: IsA<{}>{}",
            bound.alias,
            bound.type_str,
            if r#async { " + Clone + 'static" } else { "" }
        ),
        // This case should normally never happened
        AsRef(Some(_ /*lifetime*/)) => {
            unreachable!();
            // format!("{}: AsRef<{}> + '{}", bound.alias, bound.type_str, lifetime)
        }
        AsRef(None) => format!("{}: AsRef<{}>", bound.alias, bound.type_str),
    }
}

pub fn bounds(
    bounds: &Bounds,
    skip: &[char],
    r#async: bool,
    filter_callback_modified: bool,
) -> (String, Vec<String>) {
    use crate::analysis::bounds::BoundType::*;

    if bounds.is_empty() {
        return (String::new(), Vec::new());
    }

    let skip_lifetimes = bounds
        .iter()
        .filter(|bound| skip.contains(&bound.alias))
        .filter_map(|bound| match bound.bound_type {
            IsA(Some(lifetime)) | AsRef(Some(lifetime)) => Some(lifetime),
            _ => None,
        })
        .collect::<Vec<_>>();

    let strs: Vec<String> = bounds
        .iter_lifetimes()
        .filter(|s| !skip_lifetimes.contains(s))
        .map(|s| format!("'{}", s))
        .chain(
            bounds
                .iter()
                .filter(|bound| {
                    !skip.contains(&bound.alias)
                        && (!filter_callback_modified || !bound.callback_modified)
                })
                .map(|b| bound_to_string(b, r#async)),
        )
        .collect();

    if strs.is_empty() {
        (String::new(), Vec::new())
    } else {
        let bounds = bounds
            .iter_lifetimes()
            .filter(|s| !skip_lifetimes.contains(s))
            .map(|s| format!("'{}", s))
            .chain(
                bounds
                    .iter()
                    .filter(|bound| {
                        !skip.contains(&bound.alias)
                            && (!filter_callback_modified || !bound.callback_modified)
                    })
                    .map(|b| b.alias.to_string()),
            )
            .collect::<Vec<_>>();
        (format!("<{}>", strs.join(", ")), bounds)
    }
}

pub fn body_chunk(env: &Env, analysis: &analysis::functions::Info) -> Chunk {
    if analysis.visibility == Visibility::Comment {
        return ffi_function_todo(env, &analysis.glib_name);
    }

    let outs_as_return = !analysis.outs.is_empty();

    let mut builder = function_body_chunk::Builder::new();
    builder
        .glib_name(&format!(
            "{}::{}",
            env.main_sys_crate_name(),
            analysis.glib_name
        ))
        .assertion(analysis.assertion)
        .ret(&analysis.ret)
        .transformations(&analysis.parameters.transformations)
        .outs_mode(analysis.outs.mode);

    if analysis.r#async {
        if let Some(ref trampoline) = analysis.trampoline {
            builder.async_trampoline(trampoline);
        } else {
            warn!(
                "Async function {} has no associated _finish function",
                analysis.name
            );
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

pub fn body_chunk_futures(
    env: &Env,
    analysis: &analysis::functions::Info,
) -> StdResult<String, fmt::Error> {
    use crate::analysis::ref_mode::RefMode;
    use std::fmt::Write;

    let async_future = analysis.async_future.as_ref().unwrap();

    let mut body = String::new();

    if env.config.library_name != "Gio" {
        writeln!(body, "use gio::GioFuture;")?;
    } else {
        writeln!(body, "use GioFuture;")?;
    }
    writeln!(body, "use fragile::Fragile;")?;
    writeln!(body)?;

    let skip = if async_future.is_method { 1 } else { 0 };

    // Skip the instance parameter
    for par in analysis.parameters.rust_parameters.iter().skip(skip) {
        if par.name == "cancellable" || par.name == "callback" {
            continue;
        }

        let c_par = &analysis.parameters.c_parameters[par.ind_c];

        let type_ = env.type_(par.typ);
        let is_str = if let library::Type::Fundamental(library::Fundamental::Utf8) = *type_ {
            true
        } else {
            false
        };

        if *c_par.nullable {
            writeln!(
                body,
                "let {} = {}.map(ToOwned::to_owned);",
                par.name, par.name
            )?;
        } else if is_str {
            writeln!(body, "let {} = String::from({});", par.name, par.name)?;
        } else if c_par.ref_mode != RefMode::None {
            writeln!(body, "let {} = {}.clone();", par.name, par.name)?;
        }
    }

    if async_future.is_method {
        writeln!(body, "GioFuture::new(self, move |obj, send| {{")?;
    } else {
        writeln!(body, "GioFuture::new(&(), move |_obj, send| {{")?;
    }

    if env.config.library_name != "Gio" {
        writeln!(body, "\tlet cancellable = gio::Cancellable::new();")?;
    } else {
        writeln!(body, "\tlet cancellable = Cancellable::new();")?;
    }
    writeln!(body, "\tlet send = Fragile::new(send);")?;

    if async_future.is_method {
        writeln!(body, "\tobj.{}(", analysis.name)?;
    } else if analysis.type_name.is_ok() {
        writeln!(body, "\tSelf::{}(", analysis.name)?;
    } else {
        writeln!(body, "\t{}(", analysis.name)?;
    }

    // Skip the instance parameter
    for par in analysis.parameters.rust_parameters.iter().skip(skip) {
        if par.name == "cancellable" {
            writeln!(body, "\t\tSome(&cancellable),")?;
        } else if par.name == "callback" {
            continue;
        } else {
            let c_par = &analysis.parameters.c_parameters[par.ind_c];

            if *c_par.nullable {
                writeln!(
                    body,
                    "\t\t{}.as_ref().map(::std::borrow::Borrow::borrow),",
                    par.name
                )?;
            } else if c_par.ref_mode != RefMode::None {
                writeln!(body, "\t\t&{},", par.name)?;
            } else {
                writeln!(body, "\t\t{},", par.name)?;
            }
        }
    }

    writeln!(body, "\t\tmove |res| {{")?;
    writeln!(body, "\t\t\tlet _ = send.into_inner().send(res);")?;
    writeln!(body, "\t\t}},")?;
    writeln!(body, "\t);")?;
    writeln!(body)?;
    writeln!(body, "\tcancellable")?;
    writeln!(body, "}})")?;

    Ok(body)
}
