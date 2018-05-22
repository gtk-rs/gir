use std::io::{Result, Write};

use library;
use analysis;
use analysis::bounds::{BoundType, Bounds};
use analysis::ref_mode::RefMode;
use analysis::functions::Visibility;
use analysis::namespaces;
use env::Env;
use writer::primitives::tabs;
use writer::ToCode;
use codegen::parameter::ToParameter;
use chunk::{ffi_function_todo, Chunk};

use std::result::Result as StdResult;
use std::fmt;

use codegen::subclass::class_impl::SubclassInfo;
use codegen::subclass::virtual_method_body_chunks::Builder;

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

// fn func_parameters(
//     env: &Env,
//     analysis: &analysis::virtual_methods::Info,
//     bound_replace: Option<(char, &str)>,
//     closure: bool,
// ) -> String {
//     let mut param_str = String::with_capacity(100);
//
//     for (pos, par) in analysis.parameters.rust_parameters.iter().enumerate() {
//         if pos > 0 {
//             param_str.push_str(", ");
//             if !closure {
//                 param_str.push_str(&format!("{}: ", par.name));
//             }
//         } else if !closure {
//             param_str.push_str("&self");
//             continue;
//         }
//
//         let s = func_parameter(env, par, &analysis.bounds, bound_replace);
//         param_str.push_str(&s);
//     }
//
//     param_str
// }
//
// fn func_parameter(
//     env: &Env,
//     par: &RustParameter,
//     bounds: &Bounds,
//     bound_replace: Option<(char, &str)>,
// ) -> String {
//     //TODO: restore mutable support
//     //let mut_str = if par.ref_mode == RefMode::ByRefMut { "mut " } else { "" };
//     let mut_str = "";
//     let ref_mode = if par.ref_mode == RefMode::ByRefMut {
//         RefMode::ByRef
//     } else {
//         par.ref_mode
//     };
//
//     match bounds.get_parameter_alias_info(&par.name) {
//         Some((t, bound_type)) => match bound_type {
//             BoundType::NoWrapper => unreachable!(),
//             BoundType::IsA(_) => if *par.nullable {
//                 format!("&Option<{}{}>", mut_str, t)
//             } else if let Some((from, to)) = bound_replace {
//                 if from == t {
//                     format!("&{}{}", mut_str, to)
//                 } else {
//                     format!("&{}{}", mut_str, t)
//                 }
//             } else {
//                 format!("&{}{}", mut_str, t)
//             },
//             BoundType::AsRef(_) | BoundType::Into(_, _) => t.to_string(),
//         },
//         None => {
//             let rust_type =
//                 parameter_rust_type(env, par.typ, par.direction, par.nullable, ref_mode);
//             rust_type.into_string().replace("Option<&", "&Option<")
//         }
//     }
// }


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

    let body = base_impl_body_chunk(env, object_analysis, method_analysis, subclass_info).to_code(env);
    for s in body {
        try!(writeln!(w, "{}{}", tabs(indent+1), s));
    }

    try!(writeln!(
        w,
        "{}}}",
        tabs(indent),
    ));

    Ok(())
}

pub fn base_impl_body_chunk(env: &Env,
                            object_analysis: &analysis::object::Info,
                            method_analysis: &analysis::virtual_methods::Info,
                            subclass_info: &SubclassInfo
                        ) -> Chunk
{
    let mut builder = Builder::new();
    builder.object_class_c_type(object_analysis.c_class_type.as_ref().unwrap())
           .ffi_crate_name(&env.namespaces[object_analysis.type_id.ns_id].ffi_crate_name);


    builder.generate(env)
}
