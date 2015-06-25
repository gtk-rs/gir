use std::io::{Result, Write};

use analysis;
use env::Env;
use super::general;

pub fn generate<W: Write>(w: &mut W, env: &Env, class_analysis: &analysis::widget::Info) -> Result<()>{
    let class_type = class_analysis.type_(&env.library);

    try!(general::start_comments(w));
    //TODO: uses
    try!(general::objects_child_type(w, &class_analysis.type_name, &class_type.glib_name));
    //TODO: impl parents
    //TODO: impl interfaces
    //TODO: impl type
    try!(general::impl_static_type(w, &class_analysis.type_name, &class_type.get_type_func_name));
    //TODO: ext trait
    //TODO: impl trait

    Ok(())
}
