use std::io::{Result, Write};

use analysis;
use env::Env;
use super::{function, general};

pub fn generate<W: Write>(w: &mut W, env: &Env, analysis: &analysis::widget::Info) -> Result<()>{
    let type_ = analysis.type_(&env.library);

    try!(general::start_comments(w));
    //TODO: uses
    try!(general::objects_child_type(w, &analysis.name, &type_.glib_type_name));
    try!(general::impl_parents(w, &analysis.name, &analysis.parents));
    //TODO: impl interfaces
    if analysis.has_constructors {
        try!(writeln!(w, ""));
        try!(writeln!(w, "impl {} {{", analysis.name));
        for func_analysis in &analysis.constructors() {
            try!(function::generate(w, func_analysis, true, false, 1));
        }
        //TODO: methods for unchildless
        try!(writeln!(w, "}}"));
    }
    try!(general::impl_static_type(w, &analysis.name, &type_.glib_get_type));
    //TODO: ext trait
    //TODO: impl trait

    Ok(())
}
