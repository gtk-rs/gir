use std::io::{Result, Write};

use analysis;
use library;
use chunk::Chunk;
use consts::TYPE_PARAMETERS_START;
use env::Env;
use super::general::{doc_hidden, version_condition};
use super::signal_body;
use super::trampoline::func_string;
use writer::primitives::tabs;
use writer::ToCode;

pub fn generate(
    w: &mut Write,
    env: &Env,
    analysis: &analysis::signals::Info,
    trampolines: &analysis::trampolines::Trampolines,
    in_trait: bool,
    only_declaration: bool,
    indent: usize,
) -> Result<()> {
    // TODO: Add support for action signals.
    // These work the other way around than normal signals: We are supposed to emit them from
    // outside the object instead of connecting to it by using g_signal_emit*(). It basically is
    // like a dynamic function call.
    // The object itself is connected to the signal and waiting for us to emit it, to do something.
    if analysis.is_action {
        warn!("Ignoring action signal {}::{} - action signals are unsupported currently", analysis.object_name, analysis.signal_name);
        return Ok(());
    }

    let commented = analysis.trampoline_name.is_err();
    let comment_prefix = if commented { "//" } else { "" };
    let pub_prefix = if in_trait { "" } else { "pub " };

    let function_type = function_type_string(env, analysis, trampolines, true);
    let declaration = declaration(analysis, &function_type);
    let suffix = if only_declaration { ";" } else { " {" };

    try!(writeln!(w, ""));
    try!(version_condition(
        w,
        env,
        analysis.version,
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
        match function_type {
            Some(ref type_) => {
                let body = body(analysis, type_, in_trait).to_code(env);
                for s in body {
                    try!(writeln!(w, "{}{}", tabs(indent), s));
                }
            }
            _ => if let Err(ref errors) = analysis.trampoline_name {
                for error in errors {
                    try!(writeln!(w, "{}{}\t{}", tabs(indent), comment_prefix, error));
                }
                try!(writeln!(w, "{}{}}}", tabs(indent), comment_prefix));
            } else {
                try!(writeln!(
                    w,
                    "{}{}\tTODO: connect to trampoline\n{0}{1}}}",
                    tabs(indent),
                    comment_prefix
                ));
            },
        }
    }

    if let Some(ref emit_name) = analysis.action_emit_name {
        try!(writeln!(w, ""));
        try!(version_condition(
            w,
            env,
            analysis.version,
            commented,
            indent,
        ));

        let function_type = function_type_string(env, analysis, trampolines, false);

        try!(writeln!(
            w,
            "{}{}{}fn {}{}{}",
            tabs(indent),
            comment_prefix,
            pub_prefix,
            emit_name,
            function_type.unwrap(),
            suffix
        ));

        if !only_declaration {
            let trampoline_name = analysis.trampoline_name.as_ref().unwrap();
            let trampoline = match trampolines.iter().find(|t| *trampoline_name == t.name) {
                Some(trampoline) => trampoline,
                None => panic!(
                    "Internal error: can't find trampoline '{}'",
                    trampoline_name
                ),
            };
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

            try!(writeln!(
                w,
                "{}let {} = self.emit(\"{}\", &[{}]).unwrap();",
                tabs(indent + 1),
                if trampoline.ret.typ != Default::default() {
                    "res"
                } else {
                    "_"
                },
                analysis.signal_name,
                args,
            ));

            if trampoline.ret.typ != Default::default() {
                if trampoline.ret.nullable == library::Nullable(true) {
                    try!(writeln!(w, "{}res.unwrap().get()", tabs(indent + 1),));
                } else {
                    try!(writeln!(
                        w,
                        "{}res.unwrap().get().unwrap()",
                        tabs(indent + 1),
                    ));
                }
            }
            try!(writeln!(w, "{}}}", tabs(indent)));
        }
    }

    Ok(())
}

fn function_type_string(
    env: &Env,
    analysis: &analysis::signals::Info,
    trampolines: &analysis::trampolines::Trampolines,
    closure: bool,
) -> Option<String> {
    if analysis.trampoline_name.is_err() {
        return None;
    }

    let trampoline_name = analysis.trampoline_name.as_ref().unwrap();
    let trampoline = match trampolines.iter().find(|t| *trampoline_name == t.name) {
        Some(trampoline) => trampoline,
        None => panic!(
            "Internal error: can't find trampoline '{}'",
            trampoline_name
        ),
    };

    let type_ = func_string(
        env,
        trampoline,
        if closure {
            Some((TYPE_PARAMETERS_START, "Self"))
        } else {
            Some((TYPE_PARAMETERS_START, "self"))
        },
        closure,
    );
    Some(type_)
}

fn declaration(analysis: &analysis::signals::Info, function_type: &Option<String>) -> String {
    let bounds = bounds(function_type);
    let param_str = "&self, f: F";
    let return_str = " -> SignalHandlerId";
    format!(
        "fn {}<{}>({}){}",
        analysis.connect_name,
        bounds,
        param_str,
        return_str
    )
}

fn bounds(function_type: &Option<String>) -> String {
    match *function_type {
        Some(ref type_) => format!("F: {}", type_),
        _ => "Unsupported or ignored types".to_owned(),
    }
}

fn body(analysis: &analysis::signals::Info, function_type: &str, in_trait: bool) -> Chunk {
    let mut builder = signal_body::Builder::new();

    builder
        .signal_name(&analysis.signal_name)
        .trampoline_name(analysis.trampoline_name.as_ref().unwrap())
        .in_trait(in_trait)
        .function_type_string(function_type);

    builder.generate()
}
