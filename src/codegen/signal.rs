use std::io::{Result, Write};

use analysis;
use env::Env;
use super::trampoline::func_string;
use writer::primitives::tabs;

pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::signals::Info,
                trampolines: &analysis::trampolines::Trampolines,
                in_trait: bool, only_declaration: bool, indent: usize) -> Result<()> {
    let commented = analysis.trampoline_name.is_none();
    let comment_prefix = if commented { "//" } else { "" };
    let pub_prefix = if in_trait { "" } else { "pub " };
    let declaration = declaration(env, analysis, trampolines);
    let suffix = if only_declaration { ";" } else { " {" };

    try!(writeln!(w, ""));
    //TODO: version, cfg_condition
    try!(writeln!(w, "{}{}{}{}{}", tabs(indent), comment_prefix,
                  pub_prefix, declaration, suffix));

    if !only_declaration {
        //TODO: body
        match analysis.trampoline_name {
            Some(ref name) => try!(writeln!(w, "{}{}\tTODO: connect to {}",
                                            tabs(indent), comment_prefix, name)),
            None => try!(writeln!(w, "{}{}\tTODO: connect to trampoline",
                                  tabs(indent), comment_prefix)),
        }
        try!(writeln!(w, "{}{}}}", tabs(indent), comment_prefix));
    }

    Ok(())
}

fn declaration(env: &Env, analysis: &analysis::signals::Info,
                   trampolines: &analysis::trampolines::Trampolines) -> String {
    let bounds = bounds(env, analysis, trampolines);
    let param_str = "&self, f: F";
    let return_str = " -> u64";
    format!("fn {}<{}>({}){}", analysis.connect_name, bounds, param_str, return_str)
}

fn bounds(env: &Env, analysis: &analysis::signals::Info,
          trampolines: &analysis::trampolines::Trampolines) -> String {
    if analysis.trampoline_name.is_none() {
        return "Unsupported or ignored types".to_owned();
    }

    let trampoline_name = analysis.trampoline_name.as_ref().unwrap();
    let trampoline = match trampolines.iter().filter(|t| *trampoline_name == t.name).next() {
        Some(trampoline) => trampoline,
        None => panic!("Internal error: can't find trampoline '{}'", trampoline_name),
    };

    format!("F: {}", func_string(env, trampoline, Some(("T", "Self"))))
}
