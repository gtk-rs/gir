use std::io::{Result, Write};

use super::{
    general::{cfg_deprecated, doc_alias, doc_hidden, version_condition},
    signal_body,
    trampoline::{self, func_string},
};
use crate::{
    analysis,
    chunk::Chunk,
    env::Env,
    writer::{primitives::tabs, ToCode},
};

pub fn generate(
    w: &mut dyn Write,
    env: &Env,
    analysis: &analysis::signals::Info,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
) -> Result<()> {
    let commented = analysis.trampoline.is_err();
    let comment_prefix = if commented { "//" } else { "" };
    let pub_prefix = if in_trait { "" } else { "pub " };

    let function_type = function_type_string(env, analysis, true);
    let declaration = declaration(analysis, &function_type);
    let suffix = if only_declaration { ";" } else { " {" };

    writeln!(w)?;
    cfg_deprecated(w, env, None, analysis.deprecated_version, commented, indent)?;
    version_condition(w, env, None, analysis.version, commented, indent)?;
    doc_hidden(w, analysis.doc_hidden, comment_prefix, indent)?;
    // Strip the "prefix" from "prefix::prop-name", if any.
    // Ex.: "notify::is-locked".
    doc_alias(
        w,
        analysis.signal_name.splitn(2, "::").last().unwrap(),
        comment_prefix,
        indent,
    )?;
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
        if !commented {
            if let Ok(ref trampoline) = analysis.trampoline {
                trampoline::generate(w, env, trampoline, in_trait, 2)?;
            }
        }
        match function_type {
            Some(_) => {
                let body = body(analysis, in_trait).to_code(env);
                for s in body {
                    writeln!(w, "{}{}", tabs(indent), s)?;
                }
            }
            _ => {
                if let Err(ref errors) = analysis.trampoline {
                    for error in errors {
                        writeln!(w, "{}{}\t{}", tabs(indent), comment_prefix, error)?;
                    }
                    writeln!(w, "{}{}}}", tabs(indent), comment_prefix)?;
                } else {
                    writeln!(
                        w,
                        "{}{}\tTODO: connect to trampoline\n{0}{1}}}",
                        tabs(indent),
                        comment_prefix
                    )?;
                }
            }
        }
    }

    if function_type.is_none() {
        // Signal incomplete, can't generate emit
        return Ok(());
    }

    if let Some(ref emit_name) = analysis.action_emit_name {
        writeln!(w)?;
        if !in_trait || only_declaration {
            cfg_deprecated(w, env, None, analysis.deprecated_version, commented, indent)?;
        }
        version_condition(w, env, None, analysis.version, commented, indent)?;

        let function_type = function_type_string(env, analysis, false);

        writeln!(
            w,
            "{}{}{}fn {}{}{}",
            tabs(indent),
            comment_prefix,
            pub_prefix,
            emit_name,
            function_type.unwrap(),
            suffix
        )?;

        if !only_declaration {
            let trampoline = analysis.trampoline.as_ref().unwrap_or_else(|_| {
                panic!(
                    "Internal error: can't find trampoline for signal '{}'",
                    analysis.signal_name,
                )
            });
            let mut args = String::with_capacity(100);

            for (pos, par) in trampoline.parameters.rust_parameters.iter().enumerate() {
                // Skip the self parameter
                if pos == 0 {
                    continue;
                }

                if pos > 1 {
                    args.push_str(", ");
                }
                args.push('&');
                args.push_str(&par.name);
            }

            if trampoline.ret.typ != Default::default() {
                writeln!(
                    w,
                    "{}self.emit_by_name(\"{}\", &[{}])",
                    tabs(indent + 1),
                    analysis.signal_name,
                    args,
                )?;
            } else {
                writeln!(
                    w,
                    "{}self.emit_by_name::<()>(\"{}\", &[{}]);",
                    tabs(indent + 1),
                    analysis.signal_name,
                    args,
                )?;
            }
            writeln!(w, "{}}}", tabs(indent))?;
        }
    }

    Ok(())
}

fn function_type_string(
    env: &Env,
    analysis: &analysis::signals::Info,
    closure: bool,
) -> Option<String> {
    analysis.trampoline.as_ref().ok()?;

    let trampoline = analysis.trampoline.as_ref().unwrap_or_else(|_| {
        panic!(
            "Internal error: can't find trampoline for signal '{}'",
            analysis.signal_name
        )
    });

    let type_ = func_string(
        env,
        trampoline,
        Some(if closure { "Self" } else { "self" }),
        closure,
    );
    Some(type_)
}

fn declaration(analysis: &analysis::signals::Info, function_type: &Option<String>) -> String {
    let bounds = bounds(function_type);
    let param_str = if !analysis.is_detailed {
        "&self, f: F"
    } else {
        "&self, detail: Option<&str>, f: F"
    };
    let return_str = " -> SignalHandlerId";
    format!(
        "fn {}<{}>({}){}",
        analysis.connect_name, bounds, param_str, return_str
    )
}

fn bounds(function_type: &Option<String>) -> String {
    match function_type {
        Some(type_) => format!("F: {type_}"),
        _ => "Unsupported or ignored types".to_owned(),
    }
}

fn body(analysis: &analysis::signals::Info, in_trait: bool) -> Chunk {
    let mut builder = signal_body::Builder::new();

    builder
        .signal_name(&analysis.signal_name)
        .trampoline_name(&analysis.trampoline.as_ref().unwrap().name)
        .in_trait(in_trait)
        .is_detailed(analysis.is_detailed);

    builder.generate()
}
