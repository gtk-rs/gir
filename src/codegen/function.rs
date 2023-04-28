use std::{
    fmt,
    io::{Result, Write},
    result::Result as StdResult,
};

use log::warn;

use super::{
    function_body_chunk,
    general::{
        allow_deprecated, cfg_condition, cfg_deprecated, doc_alias, doc_hidden,
        not_version_condition, version_condition,
    },
    parameter::ToParameter,
    return_value::{out_parameter_types, out_parameters_as_return, ToReturnValue},
    special_functions,
};
use crate::{
    analysis::{self, bounds::Bounds, try_from_glib::TryFromGlib},
    chunk::{ffi_function_todo, Chunk},
    env::Env,
    library::{self, TypeId},
    nameutil::use_glib_type,
    version::Version,
    writer::{primitives::tabs, safety_assertion_mode_to_str, ToCode},
};

// We follow the rules of the `return_self_not_must_use` clippy lint:
//
// If `Self` is returned (so `-> Self`) in a method (whatever the form of the
// `self`), then the `#[must_use]` attribute must be added.
pub fn get_must_use_if_needed(
    parent_type_id: Option<TypeId>,
    analysis: &analysis::functions::Info,
    comment_prefix: &str,
) -> Option<String> {
    // If there is no parent, it means it's not a (trait) method so we're not
    // interested.
    if let Some(parent_type_id) = parent_type_id {
        // Check it's a trait declaration or a method declaration (outside of a trait
        // implementation).
        if analysis.kind == library::FunctionKind::Method {
            // We now get the list of the returned types.
            let outs = out_parameter_types(analysis);
            // If there is only one type returned, we check if it's the same type as `self`
            // (stored in `parent_type_id`).
            if [parent_type_id] == *outs.as_slice() {
                return Some(format!("{comment_prefix}#[must_use]\n"));
            }
        }
    }
    None
}

pub fn generate(
    w: &mut dyn Write,
    env: &Env,
    parent_type_id: Option<TypeId>,
    analysis: &analysis::functions::Info,
    special_functions: Option<&analysis::special_functions::Infos>,
    scope_version: Option<Version>,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
) -> Result<()> {
    if !analysis.status.need_generate() {
        return Ok(());
    }

    if analysis.is_async_finish(env) {
        return Ok(());
    }

    if let Some(special_functions) = special_functions {
        if special_functions::generate(w, env, analysis, special_functions, scope_version)? {
            return Ok(());
        }
    }

    if analysis.hidden {
        return Ok(());
    }

    let commented = analysis.commented;
    let comment_prefix = if commented { "//" } else { "" };
    let pub_prefix = if in_trait {
        String::new()
    } else {
        format!("{} ", analysis.visibility)
    };

    let unsafe_ = if analysis.unsafe_ { "unsafe " } else { "" };
    let declaration = declaration(env, analysis);
    let suffix = if only_declaration { ";" } else { " {" };

    writeln!(w)?;
    cfg_deprecated(w, env, None, analysis.deprecated_version, commented, indent)?;
    cfg_condition(w, analysis.cfg_condition.as_ref(), commented, indent)?;
    let version = Version::if_stricter_than(analysis.version, scope_version);
    version_condition(w, env, None, version, commented, indent)?;
    not_version_condition(w, analysis.not_version, commented, indent)?;
    doc_hidden(w, analysis.doc_hidden, comment_prefix, indent)?;
    allow_deprecated(w, analysis.deprecated_version, commented, indent)?;
    doc_alias(w, &analysis.glib_name, comment_prefix, indent)?;
    if analysis.codegen_name() != analysis.func_name {
        doc_alias(w, &analysis.func_name, comment_prefix, indent)?;
    }
    // Don't add a guard for public or copy/equal functions
    let dead_code_cfg = if !analysis.visibility.is_public() && !analysis.is_special() {
        "#[allow(dead_code)]"
    } else {
        ""
    };

    let allow_should_implement_trait = if analysis.codegen_name() == "default" {
        format!(
            "{}{}#[allow(clippy::should_implement_trait)]",
            tabs(indent),
            comment_prefix
        )
    } else {
        String::new()
    };

    writeln!(
        w,
        "{}{}{}{}{}{}{}{}{}",
        allow_should_implement_trait,
        dead_code_cfg,
        get_must_use_if_needed(parent_type_id, analysis, comment_prefix).unwrap_or_default(),
        tabs(indent),
        comment_prefix,
        pub_prefix,
        unsafe_,
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
        cfg_deprecated(w, env, None, analysis.deprecated_version, commented, indent)?;

        writeln!(w, "{}{}", tabs(indent), comment_prefix)?;
        cfg_condition(w, analysis.cfg_condition.as_ref(), commented, indent)?;
        version_condition(w, env, None, version, commented, indent)?;
        not_version_condition(w, analysis.not_version, commented, indent)?;
        doc_hidden(w, analysis.doc_hidden, comment_prefix, indent)?;
        writeln!(
            w,
            "{}{}{}{}{}{}",
            tabs(indent),
            comment_prefix,
            pub_prefix,
            unsafe_,
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
        format!(" -> Result<(), {}>", use_glib_type(env, "error::BoolError"))
    } else if let Some(return_type) = analysis.ret.to_return_value(
        env,
        analysis
            .ret
            .parameter
            .as_ref()
            .map_or(&TryFromGlib::Default, |par| &par.try_from_glib),
        false,
    ) {
        format!(" -> {return_type}")
    } else {
        String::new()
    };
    let mut param_str = String::with_capacity(100);

    let (bounds, _) = bounds(&analysis.bounds, &[], false, false);

    for par in &analysis.parameters.rust_parameters {
        if !param_str.is_empty() {
            param_str.push_str(", ");
        }
        let c_par = &analysis.parameters.c_parameters[par.ind_c];
        let s = c_par.to_parameter(env, &analysis.bounds, false);
        param_str.push_str(&s);
    }

    format!(
        "fn {}{}({}){}",
        analysis.codegen_name(),
        bounds,
        param_str,
        return_str,
    )
}

pub fn declaration_futures(env: &Env, analysis: &analysis::functions::Info) -> String {
    let async_future = analysis.async_future.as_ref().unwrap();

    let return_str = if let Some(ref error_parameters) = async_future.error_parameters {
        format!(
            " -> Pin<Box_<dyn std::future::Future<Output = Result<{}, {}>> + 'static>>",
            async_future.success_parameters, error_parameters
        )
    } else {
        format!(
            " -> Pin<Box_<dyn std::future::Future<Output = {}> + 'static>>",
            async_future.success_parameters
        )
    };

    let mut param_str = String::with_capacity(100);

    let mut skipped = 0;
    let mut skipped_bounds = vec![];
    for (pos, par) in analysis.parameters.rust_parameters.iter().enumerate() {
        let c_par = &analysis.parameters.c_parameters[par.ind_c];

        if c_par.name == "callback" || c_par.name == "cancellable" {
            skipped += 1;
            if let Some(alias) = analysis
                .bounds
                .get_parameter_bound(&c_par.name)
                .and_then(|bound| bound.type_parameter_reference())
            {
                skipped_bounds.push(alias);
            }
            continue;
        }

        if pos - skipped > 0 {
            param_str.push_str(", ");
        }

        let s = c_par.to_parameter(env, &analysis.bounds, true);
        param_str.push_str(&s);
    }

    let (bounds, _) = bounds(&analysis.bounds, skipped_bounds.as_ref(), true, false);

    format!(
        "fn {}{}({}){}",
        async_future.name, bounds, param_str, return_str,
    )
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
        // TODO: False or true?
        .filter(|bound| bound.alias.map_or(false, |alias| skip.contains(&alias)))
        .filter_map(|bound| match bound.bound_type {
            IsA(lifetime) | AsRef(lifetime) => lifetime,
            _ => None,
        })
        .collect::<Vec<_>>();

    let lifetimes = bounds
        .iter_lifetimes()
        .filter(|s| !skip_lifetimes.contains(s))
        .map(|s| format!("'{s}"))
        .collect::<Vec<_>>();

    let bounds = bounds.iter().filter(|bound| {
        bound.alias.map_or(true, |alias| !skip.contains(&alias))
            && (!filter_callback_modified || !bound.callback_modified)
    });

    let type_names = lifetimes
        .iter()
        .cloned()
        .chain(
            bounds
                .clone()
                .filter_map(|b| b.type_parameter_definition(r#async)),
        )
        .collect::<Vec<_>>();

    let type_names = if type_names.is_empty() {
        String::new()
    } else {
        format!("<{}>", type_names.join(", "))
    };

    let bounds = lifetimes
        .into_iter()
        // TODO: enforce that this is only used on NoWrapper!
        // TODO: Analyze if alias is used in function, otherwise set to None!
        .chain(bounds.filter_map(|b| b.alias).map(|a| a.to_string()))
        .collect::<Vec<_>>();

    (type_names, bounds)
}

pub fn body_chunk(env: &Env, analysis: &analysis::functions::Info) -> Chunk {
    if analysis.commented {
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
        .in_unsafe(analysis.unsafe_)
        .outs_mode(analysis.outs.mode);

    if analysis.r#async {
        if let Some(ref trampoline) = analysis.trampoline {
            builder.async_trampoline(trampoline);
        } else {
            warn!(
                "Async function {} has no associated _finish function",
                analysis.codegen_name(),
            );
        }
    } else {
        for trampoline in &analysis.callbacks {
            builder.callback(trampoline);
        }
        for trampoline in &analysis.destroys {
            builder.destroy(trampoline);
        }
    }

    for par in &analysis.parameters.c_parameters {
        if outs_as_return && analysis.outs.iter().any(|out| out.lib_par.name == par.name) {
            builder.out_parameter(env, par);
        } else {
            builder.parameter();
        }
    }

    let (bounds, bounds_names) = bounds(&analysis.bounds, &[], false, true);

    builder.generate(env, &bounds, &bounds_names.join(", "))
}

pub fn body_chunk_futures(
    env: &Env,
    analysis: &analysis::functions::Info,
) -> StdResult<String, fmt::Error> {
    use std::fmt::Write;

    use crate::analysis::ref_mode::RefMode;

    let async_future = analysis.async_future.as_ref().unwrap();

    let mut body = String::new();

    let gio_future_name = if env.config.library_name != "Gio" {
        "gio::GioFuture"
    } else {
        "crate::GioFuture"
    };
    writeln!(body)?;

    if !async_future.assertion.is_none() {
        writeln!(
            body,
            "{}",
            safety_assertion_mode_to_str(async_future.assertion)
        )?;
    }
    let skip = usize::from(async_future.is_method);

    // Skip the instance parameter
    for par in analysis.parameters.rust_parameters.iter().skip(skip) {
        if par.name == "cancellable" || par.name == "callback" {
            continue;
        }

        let c_par = &analysis.parameters.c_parameters[par.ind_c];

        let type_ = env.type_(par.typ);
        let is_str = matches!(*type_, library::Type::Basic(library::Basic::Utf8));

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
        writeln!(
            body,
            "Box_::pin({gio_future_name}::new(self, move |obj, cancellable, send| {{"
        )?;
    } else {
        writeln!(
            body,
            "Box_::pin({gio_future_name}::new(&(), move |_obj, cancellable, send| {{"
        )?;
    }

    if async_future.is_method {
        writeln!(body, "\tobj.{}(", analysis.codegen_name())?;
    } else if analysis.type_name.is_ok() {
        writeln!(body, "\tSelf::{}(", analysis.codegen_name())?;
    } else {
        writeln!(body, "\t{}(", analysis.codegen_name())?;
    }

    // Skip the instance parameter
    for par in analysis.parameters.rust_parameters.iter().skip(skip) {
        if par.name == "cancellable" {
            writeln!(body, "\t\tSome(cancellable),")?;
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
    writeln!(body, "\t\t\tsend.resolve(res);")?;
    writeln!(body, "\t\t}},")?;
    writeln!(body, "\t);")?;
    writeln!(body, "}}))")?;

    Ok(body)
}
