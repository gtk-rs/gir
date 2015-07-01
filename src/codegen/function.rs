use std::io::{Result, Write};
use std::fmt;

use analysis;
use env::Env;
use library;
use super::return_value::ToReturnValue;
use super::general::tabs;

pub fn generate<W: Write>(w: &mut W, env: &Env, analysis: &analysis::functions::Info,
    in_trait: bool, only_declaration: bool, indent: i32) -> Result<()> {

    let comment_prefix = if analysis.comented { "//" } else { "" };
    let pub_prefix = if in_trait { "" } else { "pub " };
    let declaration = declaration(&env.library, analysis);
    let suffix = if only_declaration { ";" } else { " {" };

    try!(writeln!(w, "{}{}{}{}{}", tabs(indent),
        comment_prefix, pub_prefix, declaration, suffix));

    if !only_declaration {
        if analysis.comented {
            try!(writeln!(w, "{}//{}unsafe {{ TODO: call ffi:{}() }}",
                tabs(indent), tabs(1), analysis.glib_name));
            try!(writeln!(w, "{}//}}", tabs(indent)));
        }
        else {
            let body = body(analysis);
            for s in body {
                try!(writeln!(w, "{}{}", tabs(indent + 1), s));
            }
            try!(writeln!(w, "{}}}", tabs(indent)));
        }
    }

    Ok(())
}

pub fn declaration(library: &library::Library, analysis: &analysis::functions::Info) -> String {
    let return_str = analysis.ret.to_return_value(library, analysis);
    //TODO: Trait constraints
    format!("fn {}(TODO: Params){}", analysis.name, return_str)
}

macro_rules! write_to_vec(
    ($dst:expr, $($arg:tt)*) => (
        $dst.push(fmt::format(format_args!($($arg)*)))
    )
);

pub fn body(analysis: &analysis::functions::Info) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    //TODO: real generation
    write_to_vec!(v, "unsafe {{");
    write_to_vec!(v, "{}TODO: call ffi:{}()", tabs(1), analysis.glib_name);
    write_to_vec!(v, "}}");
    v
}
