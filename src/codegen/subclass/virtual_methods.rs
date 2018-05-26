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
use traits::IntoString;
use nameutil;

use std::result::Result as StdResult;
use std::fmt;

use codegen::subclass::class_impl::SubclassInfo;
use codegen::subclass::virtual_method_body_chunks::Builder;
use codegen::sys::ffi_type::ffi_type;
use codegen::function_body_chunk::{Parameter, ReturnValue};

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

    let param_str = virtual_method_params(env, method_analysis, Some(parent_name));

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

fn virtual_method_params(env: &Env, method_analysis: &analysis::virtual_methods::Info, parent_name: Option<&String>) -> String
{
    let mut param_str = String::with_capacity(100);
    for (pos, par) in method_analysis.parameters.rust_parameters.iter().enumerate() {
        if pos > 0 {
            param_str.push_str(", ");
        }

        let c_par = &method_analysis.parameters.c_parameters[par.ind_c];

        // generate types, not trait bounds
        let bounds = Bounds::default();
        let s = c_par.to_parameter(env, &bounds);
        param_str.push_str(&s);

        // insert the templated param
        if parent_name.is_some() && pos == 0{
            param_str.push_str(&format!(", {}: &T", parent_name.as_ref().unwrap()));
        }
    }
    param_str
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

    let mut param_str = virtual_method_params(env, method_analysis, None);


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
    try!(writeln!(
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

    try!(writeln!(w, "{}}}", tabs(indent)));


    Ok(())

}

pub fn generate_box_impl(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    method_analysis: &analysis::virtual_methods::Info,
    subclass_info: &SubclassInfo,
    indent: usize,
) -> Result<()> {

    try!(writeln!(w));
    try!(write!(
        w,
        "{}fn {}(",
        tabs(indent),
        method_analysis.name,
    ));

    let parent_name = &method_analysis.parameters.rust_parameters[0].name;

    let param_str = virtual_method_params(env, method_analysis, Some(parent_name));
    try!(writeln!(w, "{}){{", param_str));


    let arg_str = virtual_method_args(method_analysis, false);


    try!(writeln!(
        w,
        "{}let imp: &$name<T> = self.as_ref();",
        tabs(indent+1)
    ));


    try!(writeln!(
        w,
        "{}imp.{}({})",
        tabs(indent+1),
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

pub fn generate_extern_c_func(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    method_analysis: &analysis::virtual_methods::Info,
    subclass_info: &SubclassInfo,
    indent: usize,
) -> Result<()> {

    try!(writeln!(w));

    try!(writeln!(
        w,
        "unsafe extern \"C\" fn {}_{}<T: {}>",
        object_analysis.name.to_lowercase(),
        method_analysis.name,
        object_analysis.subclass_base_trait_name
    ));

    let (_, sig) = function_signature(env, method_analysis, false);

    try!(writeln!(
        w,
        "{}",
        sig
    ));

    try!(writeln!(
        w,
        "where\n{}T::ImplType: {}<T>",
        tabs(indent+1),
        object_analysis.subclass_impl_trait_name
    ));
    try!(writeln!(
        w,
        "{{"
    ));

    let body = extern_c_func_body_chunk(env, object_analysis, method_analysis, subclass_info).to_code(env);
    for s in body {
        try!(writeln!(w, "{}{}", tabs(indent+1), s));
    }

    try!(writeln!(
        w,
        "}}"
    ));

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

    builder.object_name(&object_analysis.name)
           .object_class_c_type(object_analysis.c_class_type.as_ref().unwrap())
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

pub fn extern_c_func_body_chunk(env: &Env,
                            object_analysis: &analysis::object::Info,
                            method_analysis: &analysis::virtual_methods::Info,
                            subclass_info: &SubclassInfo
                        ) -> Chunk
{
    let mut builder = body_chunk_builder(env, object_analysis, method_analysis, subclass_info);

    builder.generate_extern_c_func(env)
}


pub fn function_signature(env: &Env, method: &analysis::virtual_methods::Info, bare: bool) -> (bool, String) {
    let (mut commented, ret_str) = function_return_value(env, method);

    let mut parameter_strs: Vec<String> = Vec::new();
    for par in &method.parameters.c_parameters {
        let (c, par_str) = function_parameter(env, par, bare);
        parameter_strs.push(par_str);
        if c {
            commented = true;
        }
    }

    (
        commented,
        format!("({}){}", parameter_strs.join(", "), ret_str),
    )
}

fn function_return_value(env: &Env, method: &analysis::virtual_methods::Info) -> (bool, String) {
    if  method.ret.parameter.is_none(){
        return (false, "".to_string());
    }
    let ret = method.ret.parameter.as_ref().unwrap();
    if ret.typ == Default::default() {
        return (false, String::new());
    }
    let ffi_type = ffi_type(env, ret.typ, &ret.c_type);
    let commented = ffi_type.is_err();
    (commented, format!(" -> {}", ffi_type.into_string()))
}

fn function_parameter(env: &Env, par: &analysis::function_parameters::CParameter, bare: bool) -> (bool, String) {
    if let library::Type::Fundamental(library::Fundamental::VarArgs) = *env.library.type_(par.typ) {
        return (false, "...".into());
    }
    let ffi_type = ffi_type(env, par.typ, &par.c_type);
    let commented = ffi_type.is_err();
    let res = if bare {
        ffi_type.into_string()
    } else {
        format!(
            "{}: {}",
            nameutil::mangle_keywords(&*par.name),
            ffi_type.into_string()
        )
    };
    (commented, res)
}
