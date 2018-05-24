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

pub fn default_impl_body_chunk(env: &Env,
                            object_analysis: &analysis::object::Info,
                            method_analysis: &analysis::virtual_methods::Info,
                            subclass_info: &SubclassInfo
                        ) -> Chunk
{
    let mut builder = Builder::new();
    builder.object_class_c_type(object_analysis.c_class_type.as_ref().unwrap())
           .ffi_crate_name(&env.namespaces[object_analysis.type_id.ns_id].ffi_crate_name);


    builder.generate_default_impl(env)
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

pub fn generate_override_vfuncs(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
    indent: usize,
) -> Result<()> {

    if object_analysis.c_class_type.is_none(){
        return Ok(());
    }


    try!(writeln!(w));
    try!(write!(
        w,
        "{}fn override_vfuncs(&mut self, _: &ClassInitToken){{",
        tabs(indent)
    ));

    let mut body_chunks = Vec::new();
    body_chunks.push(Chunk::Let{
        name: "klass".to_owned(),
        is_mut: false,
        value: Box::new(Chunk::Custom(format!("&mut *(self as *const Self as *mut {}::{})",
            &env.namespaces[object_analysis.type_id.ns_id].ffi_crate_name,
            object_analysis.c_class_type.as_ref().unwrap()).to_owned())),
        type_: None,
    });


    for method_analysis in &object_analysis.virtual_methods {
        body_chunks.push(Chunk::Custom(
            format!("klass.{mname} = Some({cname}_{mname}::<T>);", mname=method_analysis.name,
                                                                   cname=object_analysis.name.to_lowercase()).to_owned()
        ));
    }


    let unsafe_ = Chunk::Unsafe(body_chunks);

    let mut chunks = Vec::new();
    chunks.push(unsafe_);
    let body = Chunk::Chunks(chunks).to_code(env);

    for s in body {
        try!(writeln!(w, "{}{}", tabs(indent+1), s));
    }

    Ok(())

}



pub fn body_chunk_builder(env: &Env,
                            object_analysis: &analysis::object::Info,
                            method_analysis: &analysis::virtual_methods::Info,
                            subclass_info: &SubclassInfo
                        ) -> Builder
{
    let mut builder = Builder::new();

    let outs_as_return = !method_analysis.outs.is_empty();

    builder.object_class_c_type(object_analysis.c_class_type.as_ref().unwrap())
           .ffi_crate_name(&env.namespaces[object_analysis.type_id.ns_id].ffi_crate_name)
           .method_name(&method_analysis.name)
           .ret(&method_analysis.ret)
           .transformations(&method_analysis.parameters.transformations)
           .outs_mode(method_analysis.outs.mode);

   for par in &method_analysis.parameters.c_parameters {
       if outs_as_return && method_analysis.outs.iter().any(|p| p.name == par.name) {
           builder.out_parameter(env, par);
       } else {
           builder.parameter();
       }
   }

    builder
}

pub fn base_impl_body_chunk(env: &Env,
                            object_analysis: &analysis::object::Info,
                            method_analysis: &analysis::virtual_methods::Info,
                            subclass_info: &SubclassInfo
                        ) -> Chunk
{
    let mut builder = body_chunk_builder(env, object_analysis, method_analysis, subclass_info);

    builder.generate_base_impl(env)
}
