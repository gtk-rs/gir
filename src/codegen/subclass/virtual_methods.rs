use std::io::{Result, Write};

use library;
use analysis;
use analysis::bounds::Bounds;
use analysis::functions::Visibility;
use analysis::namespaces;
use env::Env;
use writer::primitives::tabs;
use writer::ToCode;
use codegen::parameter::ToParameter;

use std::result::Result as StdResult;
use std::fmt;

use codegen::subclass::class_impl::SubclassInfo;

pub fn generate_default_impl(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    method_analysis: &analysis::virtual_methods::Info,
    subclass_info: &SubclassInfo,
    indent: usize,
) -> Result<()> {
    info!("vfunc: {:?}", method_analysis.name);


    try!(writeln!(w));
    try!(write!(
        w,
        "{}fn {}(",
        tabs(indent),
        method_analysis.name,
    ));

    let parent_name = &method_analysis.parameters.rust_parameters[0].name;


    let mut param_str = String::with_capacity(100);
    for (pos, par) in method_analysis.parameters.rust_parameters.iter().enumerate() {
        if pos > 0 {
            param_str.push_str(", ");
        }

        let c_par = &method_analysis.parameters.c_parameters[par.ind_c];
        let s = c_par.to_parameter(env, &method_analysis.bounds);
        param_str.push_str(&s);

        // insert the templated param
        if pos == 0{
            param_str.push_str(&format!(", {}: &T", parent_name));
        }
    }


    try!(writeln!(w, "{}){{", param_str));


    let arg_str = virtual_method_args(method_analysis, false);

    try!(writeln!(
        w,
        "{}{}.parent_{}({})",
        tabs(indent+1),
        parent_name,
        method_analysis.name,
        arg_str
    ));


    try!(writeln!(
        w,
        "{}}}",
        tabs(indent),
    ));

    Ok(())

}


fn virtual_method_args(method_analysis: &analysis::virtual_methods::Info, include_parent: bool) -> String
{
    let mut arg_str = String::with_capacity(100);
    for (pos, par) in method_analysis.parameters.rust_parameters.iter().enumerate() {
        if !include_parent && pos == 0{
            // skip the first one,
            continue;
        }
        if pos > 1 {
            arg_str.push_str(", ");
        }
        arg_str.push_str(&par.name);
    }
    arg_str
}


pub fn generate_base_impl(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    method_analysis: &analysis::virtual_methods::Info,
    subclass_info: &SubclassInfo,
    indent: usize,
) -> Result<()> {
    info!("vfunc: {:?}", method_analysis.name);


    try!(writeln!(w));
    try!(write!(
        w,
        "{}fn parent_{}(",
        tabs(indent),
        method_analysis.name,
    ));

    let mut param_str = String::with_capacity(100);
    for (pos, par) in method_analysis.parameters.rust_parameters.iter().enumerate() {
        if pos > 0 {
            param_str.push_str(", ");
        }

        let c_par = &method_analysis.parameters.c_parameters[par.ind_c];
        let s = c_par.to_parameter(env, &method_analysis.bounds);
        param_str.push_str(&s);

    }


    try!(writeln!(w, "{}){{", param_str));



    // fn parent_startup(&self) {
    //     unsafe {
    //         let klass = self.get_class();
    //         let parent_klass = (*klass).get_parent_class() as *const gio_ffi::GApplicationClass;
    //         (*parent_klass)
    //             .startup
    //             .map(|f| f(self.to_glib_none().0))
    //             .unwrap_or(())
    //     }
    // }


    // try!(writeln!(
    //     w,
    //     "{}{}.parent_{}({})",
    //     tabs(indent+1),
    //     parent_name,
    //     method_analysis.name,
    //     arg_str
    // ));


    try!(writeln!(
        w,
        "{}}}",
        tabs(indent),
    ));

    Ok(())

}
