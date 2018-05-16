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

use std::result::Result as StdResult;
use std::fmt;

use codegen::subclass::object::SubclassInfo;

// pub fn generate_impl -->
//  pub trait ApplicationImpl<T: ApplicationBase>: ObjectImpl<T> + AnyImpl + 'static {

pub fn generate_impl(w: &mut Write,
                     env: &Env,
                     analysis: &analysis::object::Info,
                     subclass_info: &SubclassInfo
                 ) -> Result<()> {

    // start impl trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub trait {}<T: {}>: ObjectImpl<T> + AnyImpl + 'static {{",
        analysis.subclass_impl_trait_name,
        analysis.subclass_base_trait_name
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

    // start base trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub unsafe trait {}: IsA<{}> + ObjectType {{",
        analysis.subclass_base_trait_name,
        analysis.subclass_impl_trait_name //TODO: user-facing parent
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
