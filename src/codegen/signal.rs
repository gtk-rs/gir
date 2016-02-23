use std::io::{Result, Write};

use analysis;
use writer::primitives::tabs;

pub fn generate(w: &mut Write, analysis: &analysis::signals::Info,
    in_trait: bool, only_declaration: bool, indent: usize) -> Result<()> {
    let comment_prefix = "//";
    let pub_prefix = if in_trait { "" } else { "pub " };
    let declaration = declaration(analysis);
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
            None => try!(writeln!(w, "{}{}\tTODO: connect to unknown trampoline",
                                  tabs(indent), comment_prefix)),
        }
        try!(writeln!(w, "{}{}}}", tabs(indent), comment_prefix));
    }

    Ok(())
}

pub fn declaration(analysis: &analysis::signals::Info) -> String {
    let bounds = "/*TODO: bounds*/";
    let param_str = "&self, f: F";
    let return_str = " -> u64";
    format!("fn {}<{}>({}){}", analysis.connect_name, bounds, param_str, return_str)
}
