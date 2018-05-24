use std::io::{Result, Write};

use analysis;
use analysis::bounds::Bounds;
use analysis::functions::Visibility;
use analysis::namespaces;
use chunk::{ffi_function_todo, Chunk};
use env::Env;
use library;

use nameutil;
use writer::primitives::tabs;
use writer::ToCode;

use std::fmt;
use std::result::Result as StdResult;

use codegen::general;
use codegen::subclass::virtual_methods;
use codegen::sys::fields;
use library::*;

use analysis::general::StatusedTypeId;

pub struct SubclassInfo {
    parents: Vec<StatusedTypeId>
}

impl SubclassInfo {
    pub fn new(env: &Env, analysis: &analysis::object::Info) -> Self {
        let parents = analysis
            .supertypes
            .iter()
            .filter(|t| match *env.library.type_(t.type_id) {
                library::Type::Class(..) => true,
                library::Type::Interface(..) => true,
                _ => false,
            })
            .cloned()
            .collect::<Vec<_>>();

        Self {
            parents,
        }
    }

    fn parent_names(&self, env: &Env, krate_suffix: &str) -> Vec<String> {
        self.parents
            .iter()
            .map(|ref p| {
                if p.type_id.ns_id == namespaces::MAIN {
                    p.name.clone()
                } else {
                    format!(
                        "{krate}{krate_suffix}::{name}",
                        krate = env.namespaces[p.type_id.ns_id].crate_name,
                        krate_suffix = krate_suffix,
                        name = p.name
                    )
                }
            })
            .collect()
    }

}

pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
    try!(general::start_comments(w, &env.config));
    try!(general::uses(w, env, &analysis.imports));
    // TODO: insert gobject-subclass uses

    let subclass_info = SubclassInfo::new(env, analysis);

    try!(generate_impl(w, env, analysis, &subclass_info));

    try!(generate_impl_ext(w, env, analysis, &subclass_info));

    try!(generate_any_impl(w, env, analysis, &subclass_info));

    try!(generate_base(w, env, analysis, &subclass_info));

    try!(generate_ext(w, env, analysis, &subclass_info));


    Ok(())
}

// pub fn generate_impl -->
//  pub trait ApplicationImpl<T: ApplicationBase>: ObjectImpl<T> + AnyImpl + 'static {

pub fn generate_impl(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {

    let mut parents = subclass_info.parent_names(env, "_subclass");

    let parent_impls: Vec<String> = parents
        .iter()
        .map(|ref p| format!(" {}Impl<T> +", p))
        .collect();
    let parent_objs = parent_impls.join("");

    // start impl trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub trait {}<T: {}>:{} ObjectImpl<T> + AnyImpl + 'static {{",
        object_analysis.subclass_impl_trait_name,
        object_analysis.subclass_base_trait_name,
        parent_objs
    ));

    info!("supertypes, {:?}", parents);

    for method_analysis in &object_analysis.virtual_methods {
        try!(virtual_methods::generate_default_impl(
            w,
            env,
            object_analysis,
            method_analysis,
            subclass_info,
            1
        ));
    }

    //end impl trait
    try!(writeln!(w));
    try!(writeln!(w, "}}"));

    Ok(())
}

pub fn generate_impl_ext(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {


    let implext_name = format!("{}Ext", object_analysis.subclass_impl_trait_name);

    // start ext trait def
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub trait {}<T> {{}}",
        implext_name
    ));

    //end ext trait def
    try!(writeln!(w));
    try!(writeln!(w, "}}"));


    // start ext trait impl
    let parents = subclass_info.parent_names(env, "");

    let parent_impls: Vec<String> = parents
        .iter()
        .map(|ref p| format!("+ glib::IsA<{}>", p))
        .collect();
    let parent_objs = parent_impls.join(" ");


    try!(writeln!(
        w,
        "impl<S: {impl_name}<T>, T: ObjectType {parents}>> {implext_name}<T> for S {{}}",
        impl_name = object_analysis.subclass_impl_trait_name,
        parents = parent_objs,
        implext_name = implext_name
    ));


    Ok(())
}



pub fn generate_base(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {

    let parents = subclass_info.parent_names(env, "");

    let parent_impls: Vec<String> = parents
        .iter()
        .map(|ref p| format!("+ glib::IsA<{}>", p))
        .collect();
    let parent_objs = parent_impls.join(" ");

    // start base trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub unsafe trait {}: ObjectType {}{{",
        object_analysis.subclass_base_trait_name,
        parent_objs
    ));

    for method_analysis in &object_analysis.virtual_methods {
        try!(virtual_methods::generate_base_impl(
            w,
            env,
            object_analysis,
            method_analysis,
            subclass_info,
            1
        ));
    }

    //end base trait
    try!(writeln!(w));
    try!(writeln!(w, "}}"));

    Ok(())
}

// pub fn generate_base -->
// pub unsafe trait ApplicationBase: IsA<gio::Application> + ObjectType {

fn generate_any_impl(
    w: &mut Write,
    _env: &Env,
    object_analysis: &analysis::object::Info,
    _subclass_info: &SubclassInfo,
) -> Result<()> {
    try!(writeln!(w));
    try!(writeln!(
        w,
        "any_impl!({}, {});",
        object_analysis.subclass_base_trait_name,
        object_analysis.subclass_impl_trait_name
    ));

    Ok(())
}


fn generate_ext(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {

    if object_analysis.class_type.is_none(){
        return Ok(());
    }


    let classext_name = format!("{}Ext", object_analysis.class_type.as_ref().unwrap());

    // start base trait
    try!(writeln!(w));
    try!(writeln!(
        w,
        "pub unsafe trait {}<T: {}>
        where
        T::ImplType: {}<T>{{",
        classext_name,
        object_analysis.subclass_base_trait_name,
        object_analysis.subclass_impl_trait_name
    ));


    try!(virtual_methods::generate_override_vfuncs(
        w,
        env,
        object_analysis,
        subclass_info,
        1
    ));

    try!(writeln!(w));
    try!(writeln!(w, "}}"));

    Ok(())
}
