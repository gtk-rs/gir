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
use codegen::subclass::functions;
use codegen::general;
use codegen::sys::fields;


pub fn generate(w: &mut Write, env: &Env, class: &Class, analysis: &analysis::object::Info) -> Result<()>
{
    try!(general::start_comments(w, &env.config));
    try!(general::uses(w, env, &analysis.imports));
    // TODO: insert gobject-subclass uses
    // TODO: insert gobject-subclass uses of parent types

    info!("{:?}, {:?}", analysis.c_type, analysis.c_type);

    let subclass_info = SubclassInfo::new(env, class, analysis);

    generate_impl(w, env, class, analysis, &subclass_info);

    generate_impl_ext(w, env, class, analysis, &subclass_info);

    generate_any_impl(w, env, class, analysis, &subclass_info);


    generate_base(w, env, class, analysis, &subclass_info);


    Ok(())
}

// pub fn generate_impl -->
//  pub trait ApplicationImpl<T: ApplicationBase>: ObjectImpl<T> + AnyImpl + 'static {




pub fn generate_impl(w: &mut Write,
                     env: &Env,
                     class: &Class,
                     analysis: &analysis::object::Info,
                     subclass_info: &SubclassInfo
                 ) -> Result<()> {


    if analysis.class_type.is_some(){
        // let name = format!("{}.{}", env.config., &analysis.class_type.as_ref().unwrap());
        // let klass = env.config.objects.vmap(|x| x.name).collect();
        let ns: Vec<String> = env.library.namespaces.iter().map(|ref x| x.name.clone()).collect();
       info!("Generating  {:?} {:?}", ns, env.analysis.objects.keys());

       // let f = fields::from_class()
    }



    // start impl trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub trait {}<T: {}>: ObjectImpl<T> + AnyImpl + 'static {{", //TODO: use real superclasses chain
        analysis.subclass_impl_trait_name,
        analysis.subclass_base_trait_name
    ));

    for func_analysis in &analysis.functions{
        try!(functions::generate_impl(w, env, func_analysis, subclass_info, 1));
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
                     class: &Class,
                     analysis: &analysis::object::Info,
                     subclass_info: &SubclassInfo
                 ) -> Result<()> {

    // start impl trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub trait {}Ext<T> {{}}",
        analysis.subclass_impl_trait_name
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
                     class: &Class,
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


fn generate_any_impl(w: &mut Write, _env: &Env, class: &Class, analysis: &analysis::object::Info, _subclass_info: &SubclassInfo) -> Result<()>
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
