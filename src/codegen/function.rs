use std::io::{Result, Write};

use analysis;
use library;
use super::general::tabs;

pub fn generate<W: Write>(w: &mut W, analysis: &analysis::functions::Info,
    is_pub: bool, only_declaration: bool, indent: i32) -> Result<()> {

    let comment_prefix = if analysis.comented { "//" } else { "" };
    let pub_prefix = if is_pub { "pub " } else { "" };
    let declaration = declaration(analysis);
    let suffix = if only_declaration { ";" } else { " {" };

    try!(writeln!(w, "{}{}{}{}{}", tabs(indent),
        comment_prefix, pub_prefix, declaration, suffix));

    if !only_declaration {
        try!(writeln!(w, "{}unsafe {{", tabs(indent + 1)));
        try!(writeln!(w, "{}TODO: Body", tabs(indent + 2)));
        try!(writeln!(w, "{}}}", tabs(indent + 1)));
        try!(writeln!(w, "{}}}", tabs(indent)));
    }

    Ok(())
}

pub fn declaration(analysis: &analysis::functions::Info) -> String {
    //TODO: Optional constructors if any
    //TODO: return values
    let return_str = if analysis.kind == library::FunctionKind::Constructor {
        " -> Self"  //TODO: actual type
    } else {
        "TODO"
    };
    format!("fn {}(TODO: Params){}", analysis.name, return_str)
}
