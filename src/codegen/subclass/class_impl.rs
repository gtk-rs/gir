use std::io::{Result, Write};

use library;
use analysis;
use analysis::bounds::Bounds;
use analysis::functions::Visibility;
use analysis::namespaces;
use chunk::{ffi_function_todo, Chunk};
use env::Env;

use writer::primitives::tabs;
use writer::ToCode;
use nameutil;

use std::result::Result as StdResult;
use std::fmt;

use library::*;
use codegen::subclass::class_impls::SubclassInfo;
use codegen::subclass::virtual_methods;
use codegen::general;
use codegen::sys::fields;


pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::object::Info) -> Result<()>
{
    try!(general::start_comments(w, &env.config));
    try!(general::uses(w, env, &analysis.imports));
    // TODO: insert gobject-subclass uses
    // TODO: insert gobject-subclass uses of parent types

    info!("{:?}, {:?}", analysis.c_type, analysis.c_type);

    let subclass_info = SubclassInfo::new(env, analysis);

    generate_impl(w, env, analysis, &subclass_info);

    generate_impl_ext(w, env, analysis, &subclass_info);

    generate_any_impl(w, env, analysis, &subclass_info);


    generate_base(w, env, analysis, &subclass_info);


    Ok(())
}

// pub fn generate_impl -->
//  pub trait ApplicationImpl<T: ApplicationBase>: ObjectImpl<T> + AnyImpl + 'static {




pub fn generate_impl(w: &mut Write,
                     env: &Env,
                     object_analysis: &analysis::object::Info,
                     subclass_info: &SubclassInfo
                 ) -> Result<()> {


    // start impl trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub trait {}<T: {}>: ObjectImpl<T> + AnyImpl + 'static {{", //TODO: use real superclasses chain
        object_analysis.subclass_impl_trait_name,
        object_analysis.subclass_base_trait_name
    ));

    for method_analysis in &object_analysis.virtual_methods{
        try!(virtual_methods::generate_default_impl(w, env, object_analysis, method_analysis, subclass_info, 1));


    }

    //end impl trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "}}"
    ));

    Ok(())
}

pub fn generate_impl_ext(w: &mut Write,
                     env: &Env,
                     object_analysis: &analysis::object::Info,
                     subclass_info: &SubclassInfo
                 ) -> Result<()> {

    // start impl trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub trait {}Ext<T> {{}}",
        object_analysis.subclass_impl_trait_name
    ));

    //end impl trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "}}"
    ));

    Ok(())
}


pub fn generate_base(w: &mut Write,
                     env: &Env,
                     analysis: &analysis::object::Info,
                     subclass_info: &SubclassInfo
                 ) -> Result<()> {

    let normal_crate_name = nameutil::crate_name(&env.config.library_name);

    // start base trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub unsafe trait {}: IsA<{}::{}> + ObjectType {{",
        analysis.subclass_base_trait_name,
        normal_crate_name,
        analysis.name
    ));

    //end base trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "}}"
    ));

    Ok(())
}

// pub fn generate_base -->
// pub unsafe trait ApplicationBase: IsA<gio::Application> + ObjectType {


fn generate_any_impl(w: &mut Write, _env: &Env, analysis: &analysis::object::Info, _subclass_info: &SubclassInfo) -> Result<()>
{
    try!(writeln!(w));
    try!(writeln!(
        w,
        "any_impl!({}, {});",
        analysis.subclass_base_trait_name,
        analysis.subclass_impl_trait_name
    ));

    Ok(())
}
