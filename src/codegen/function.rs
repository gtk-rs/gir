use std::io::{Result, Write};

use analysis;
use library;
use super::general::tabs;

pub fn generate<W: Write>(w: &mut W, analysis: &analysis::functions::Info,
    in_trait: bool, only_declaration: bool, indent: i32) -> Result<()> {

    let comment_prefix = if analysis.comented { "//" } else { "" };
    let pub_prefix = if in_trait { "" } else { "pub " };
    let declaration = declaration(analysis);
    let suffix = if only_declaration { ";" } else { " {" };

    try!(writeln!(w, "{}{}{}{}{}", tabs(indent),
        comment_prefix, pub_prefix, declaration, suffix));

    if !only_declaration {
        let body = body(analysis);
        for s in body {
            try!(writeln!(w, "{}{}", tabs(indent + 1), s));
        }
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

pub fn body(analysis: &analysis::functions::Info) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    //TODO: real generation
    //TODO: use trait Write
    v.push("unsafe {".into());
    v.push(format!("{}TODO: call ffi:{}()", tabs(1), analysis.glib_name).into());
    v.push("}".into());
    v
}
