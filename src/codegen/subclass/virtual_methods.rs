use std::io::{Result, Write};

use library;
use analysis;
use analysis::bounds::Bounds;
use analysis::functions::Visibility;
use analysis::namespaces;
use env::Env;
use writer::primitives::tabs;
use writer::ToCode;

use std::result::Result as StdResult;
use std::fmt;

use codegen::subclass::class_impls::SubclassInfo;

pub fn generate_default_impl(
    w: &mut Write,
    env: &Env,
    analysis: &analysis::object::Info,
    method: &library::Signal,
    subclass_info: &SubclassInfo,
    indent: usize,
) -> Result<()> {
    info!("vfunc: {:?}", method.name);

//     fn window_added(&self, application: &T, window: &gtk::Window) {
//     application.parent_window_added(window)
// }
    try!(writeln!(w));
    try!(writeln!(
        w,
        "{}fn {}(&self, {}:&T){{",
        tabs(indent),
        method.name,
        analysis.name.to_lowercase(),
    ));

    try!(writeln!(
        w,
        "{}{}.parent_{}()",
        tabs(indent+1),
        analysis.name.to_lowercase(),
        method.name
    ));


    try!(writeln!(
        w,
        "{}}}",
        tabs(indent),
    ));

    Ok(())


}
