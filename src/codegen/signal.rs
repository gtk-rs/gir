use std::io::{Result, Write};

use analysis;
use chunk::Chunk;
use env::Env;
use super::general::version_condition;
use super::signal_body;
use super::trampoline::func_string;
use writer::primitives::tabs;
use writer::ToCode;

pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::signals::Info,
                trampolines: &analysis::trampolines::Trampolines,
                in_trait: bool, only_declaration: bool, indent: usize) -> Result<()> {
    let commented = analysis.trampoline_name.is_err();
    let comment_prefix = if commented { "//" } else { "" };
    let pub_prefix = if in_trait { "" } else { "pub " };

    let function_type_string = function_type_string(env, analysis, trampolines);
    let declaration = declaration(analysis, &function_type_string);
    let suffix = if only_declaration { ";" } else { " {" };

    try!(writeln!(w, ""));
    try!(version_condition(w, env, analysis.version, commented, indent));
    try!(writeln!(w, "{}{}{}{}{}", tabs(indent), comment_prefix,
                  pub_prefix, declaration, suffix));

    if !only_declaration {
        match function_type_string {
            Some(ref type_) => {
                let body = body(analysis, &type_, in_trait).to_code(env);
                for s in body {
                    try!(writeln!(w, "{}{}", tabs(indent), s));
                }
            }
            _ => {
                if let Err(ref errors) = analysis.trampoline_name {
                    for error in errors {
                        try!(writeln!(w, "{}{}\t{}", tabs(indent), comment_prefix, error));
                    }
                    try!(writeln!(w, "{}{}}}", tabs(indent), comment_prefix));
                } else {
                    try!(writeln!(w, "{}{}\tTODO: connect to trampoline\n{0}{1}}}",
                                  tabs(indent), comment_prefix));
                }
            }
        }
    }

    Ok(())
}

fn function_type_string(env: &Env, analysis: &analysis::signals::Info,
                        trampolines: &analysis::trampolines::Trampolines)-> Option<String> {
    if analysis.trampoline_name.is_err() {
        return None;
    }

    let trampoline_name = analysis.trampoline_name.as_ref().unwrap();
    let trampoline = match trampolines.iter().filter(|t| *trampoline_name == t.name).next() {
        Some(trampoline) => trampoline,
        None => panic!("Internal error: can't find trampoline '{}'", trampoline_name),
    };

    let type_ = func_string(env, trampoline, Some(("T", "Self")));
    Some(type_)
}

fn declaration(analysis: &analysis::signals::Info,
               function_type_string: &Option<String>) -> String {
    let bounds = bounds(function_type_string);
    let param_str = "&self, f: F";
    let return_str = " -> u64";
    format!("fn {}<{}>({}){}", analysis.connect_name, bounds, param_str, return_str)
}

fn bounds(function_type_string: &Option<String>) -> String {
    match *function_type_string {
        Some(ref type_) => format!("F: {}", type_),
        _ =>  return "Unsupported or ignored types".to_owned(),
    }
}

fn body(analysis: &analysis::signals::Info, function_type_string: &str,
        in_trait: bool) -> Chunk {
    let mut builder = signal_body::Builder::new();

    builder.signal_name(&analysis.signal_name)
        .trampoline_name(&analysis.trampoline_name.as_ref().unwrap())
        .in_trait(in_trait)
        .function_type_string(function_type_string);

    builder.generate()
}
