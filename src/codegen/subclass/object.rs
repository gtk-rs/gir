use std::io::{Result, Write};

use analysis;
use library;
use env::Env;
use codegen::child_properties;
use codegen::function;
use codegen::general;
use codegen::properties;
use codegen::signal;
use codegen::trait_impls;
use codegen::trampoline;


use codegen::subclass::traits;


pub struct SubclassInfo{

}

impl SubclassInfo{
    pub fn new(env: &Env, analysis: &analysis::object::Info) -> Self{
        Self{}
    }
}


pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
    try!(general::start_comments(w, &env.config));
    try!(general::uses(w, env, &analysis.imports));
    // TODO: insert gobject-subclass uses
    // TODO: insert gobject-subclass uses of parent types

    println!("{:?}, {:?}", analysis.subclass_impl_trait_name, analysis.subclass_base_trait_name);

    let subclass_info = SubclassInfo::new(env, analysis);

    traits::generate_impl(w, env, analysis, &subclass_info);


    traits::generate_base(w, env, analysis, &subclass_info);


    Ok(())
}
