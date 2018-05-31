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
use codegen::return_value::{ToReturnValue, out_parameter_as_return};


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

    let parent_name = &method_analysis.parameters.rust_parameters[0].name;
    let declr = declaration(env, method_analysis, None, Some(parent_name));

    try!(writeln!(
        w,
        "{}{}{{",
        tabs(indent),
        declr,
    ));


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

pub fn generate_declaration(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    method_analysis: &analysis::virtual_methods::Info,
    subclass_info: &SubclassInfo,
    indent: usize,
) -> Result<()> {

    try!(writeln!(w));

    let parent_name = &method_analysis.parameters.rust_parameters[0].name;
    let declr = declaration(env, method_analysis, None, Some(parent_name));

    try!(writeln!(
        w,
        "{}{};",
        tabs(indent),
        declr,
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


pub fn declaration(env: &Env, method_analysis: &analysis::virtual_methods::Info, method_name: Option<&String>, parent_name: Option<&String>) -> String {
    let outs_as_return = !method_analysis.outs.is_empty();
    let return_str = if outs_as_return {
        out_parameters_as_return(env, method_analysis)
    } else if method_analysis.ret.bool_return_is_error.is_some() {
        if env.namespaces.glib_ns_id == namespaces::MAIN {
            " -> Result<(), error::BoolError>".into()
        } else {
            " -> Result<(), glib::error::BoolError>".into()
        }
    } else {
        method_analysis.ret.to_return_value(env)
    };
    let mut param_str = String::with_capacity(100);

    // generate types, not trait bounds
    let bounds = Bounds::default();
    for (pos, par) in method_analysis.parameters.rust_parameters.iter().enumerate() {
        if pos > 0 {
            param_str.push_str(", ")
        }
        let c_par = &method_analysis.parameters.c_parameters[par.ind_c];
        let s = c_par.to_parameter(env, &bounds);
        param_str.push_str(&s);

        // insert the templated param
        if parent_name.is_some() && pos == 0{
            param_str.push_str(&format!(", {}: &T", parent_name.as_ref().unwrap()));
        }
    }

    format!(
        "fn {}({}){}",
        method_name.unwrap_or(&method_analysis.name),
        param_str,
        return_str
    )
}


pub fn out_parameter_as_return_parts(
    analysis: &analysis::virtual_methods::Info,
) -> (&'static str, &'static str) {
    use analysis::out_parameters::Mode::*;
    let num_outs = analysis
        .outs
        .iter()
        .filter(|p| p.array_length.is_none())
        .count();
    match analysis.outs.mode {
        Normal | Combined => if num_outs > 1 {
            ("(", ")")
        } else {
            ("", "")
        },
        Optional => if num_outs > 1 {
            ("Option<(", ")>")
        } else {
            ("Option<", ">")
        },
        Throws(..) => {
            if num_outs == 1 + 1 {
                //if only one parameter except "glib::Error"
                ("Result<", ", Error>")
            } else {
                ("Result<(", "), Error>")
            }
        }
        None => unreachable!(),
    }
}

pub fn out_parameters_as_return(env: &Env, analysis: &analysis::virtual_methods::Info) -> String {
    let (prefix, suffix) = out_parameter_as_return_parts(analysis);
    let mut return_str = String::with_capacity(100);
    return_str.push_str(" -> ");
    return_str.push_str(prefix);

    let array_lengths: Vec<_> = analysis
        .outs
        .iter()
        .filter_map(|p| p.array_length)
        .collect();

    let mut skip = 0;
    for (pos, par) in analysis.outs.iter().filter(|par| !par.is_error).enumerate() {
        // The actual return value is inserted with an empty name at position 0
        if !par.name.is_empty() {
            let mangled_par_name = nameutil::mangle_keywords(par.name.as_str());
            let param_pos = analysis
                .parameters
                .c_parameters
                .iter()
                .enumerate()
                .filter_map(|(pos, orig_par)| if orig_par.name == mangled_par_name {
                    Some(pos)
                } else {
                    None
                })
                .next()
                .unwrap();
            if array_lengths.contains(&(param_pos as u32)) {
                skip += 1;
                continue;
            }
        }

        if pos > skip {
            return_str.push_str(", ")
        }
        let s = out_parameter_as_return(par, env);
        return_str.push_str(&s);
    }
    return_str.push_str(suffix);
    return_str
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

    let declr = declaration(env, method_analysis, Some(&format!("parent_{}", method_analysis.name)), None);
    try!(writeln!(
        w,
        "{}{}{{",
        tabs(indent),
        declr,
    ));

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


    let parent_name = &method_analysis.parameters.rust_parameters[0].name;
    let declr = declaration(env, method_analysis, None, Some(parent_name));

    try!(writeln!(
        w,
        "{}{}{{",
        tabs(indent),
        declr,
    ));

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

    // TODO: use Chunk::ExternCFunc

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
           .glib_name(&method_analysis.glib_name)
           .method_name(&method_analysis.name)
           .assertion(method_analysis.assertion)
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
    for (pos, par) in method.parameters.c_parameters.iter().enumerate() {

        let param_name = if pos == 0 { Some("ptr".to_owned()) } else { None };

        let (c, par_str) = function_parameter(env, par, bare, param_name.as_ref());
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

fn function_parameter(env: &Env, par: &analysis::function_parameters::CParameter, bare: bool, param_name: Option<&String>) -> (bool, String) {
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
            param_name.unwrap_or(&nameutil::mangle_keywords(&*par.name).into_owned()),
            ffi_type.into_string()
        )
    };
    (commented, res)
}

pub fn generate_interface_init(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
    indent: usize,
) -> Result<()> {

    try!(writeln!(
        w,
        "
unsafe extern \"C\" fn {}_init<T: ObjectType>
    iface: glib_ffi::gpointer,
    iface_data: glib_ffi::gpointer,
) {",
        object_analysis.name.to_lowercase()
    ));

    // unsafe extern "C" fn uri_handler_init<T: ObjectType>(
    //     iface: glib_ffi::gpointer,
    //     iface_data: glib_ffi::gpointer,
    // ) {
    //     callback_guard!();
    //     let uri_handler_iface = &mut *(iface as *mut gst_ffi::GstURIHandlerInterface);
    //
    //     let iface_type = (*(iface as *const gobject_ffi::GTypeInterface)).g_type;
    //     let type_ = (*(iface as *const gobject_ffi::GTypeInterface)).g_instance_type;
    //     let klass = &mut *(gobject_ffi::g_type_class_ref(type_) as *mut ClassStruct<T>);
    //     let interfaces_static = &mut *(klass.interfaces_static as *mut Vec<_>);
    //     interfaces_static.push((iface_type, iface_data));
    //
    //     uri_handler_iface.get_type = Some(uri_handler_get_type::<T>);
    //     uri_handler_iface.get_protocols = Some(uri_handler_get_protocols::<T>);
    //     uri_handler_iface.get_uri = Some(uri_handler_get_uri::<T>);
    //     uri_handler_iface.set_uri = Some(uri_handler_set_uri::<T>);
    // }

    try!(writeln!(w,"}}"));

    Ok(())

}
