use std::io::{Result, Write};
use std::fs;

use analysis;
use analysis::bounds::Bounds;
use analysis::functions::Visibility;
use analysis::namespaces;
use chunk::{ffi_function_todo, Chunk};
use env::Env;
use library;
use config::ExternalLibrary;
use nameutil;
use writer::primitives::tabs;
use writer::ToCode;

use std::fmt;
use std::result::Result as StdResult;

use codegen::general;
use codegen::subclass::{virtual_methods, statics};
use codegen::sys::fields;
use codegen::sys::statics as statics_ffi;

use library::*;

use analysis::general::StatusedTypeId;

pub struct SubclassInfo {
    parents: Vec<StatusedTypeId>,
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

        Self { parents }
    }

    fn parent_names(&self, env: &Env, krate_suffix: &str) -> Vec<String> {
        self.parents
            .iter()
            .map(|ref p| /*{
                if p.type_id.ns_id == namespaces::MAIN {
                    p.name.clone()
                } else*/ {
                    format!(
                        "{krate}{krate_suffix}::{name}",
                        krate = env.namespaces[p.type_id.ns_id].crate_name,
                        krate_suffix = krate_suffix,
                        name = p.name
                    )
                //}
            })
            .collect()
    }

    fn parent<'a>(&self, env: &'a Env) -> Option<&'a analysis::object::Info>{
        // get the actual superclass object
        if self.parents.len() == 0 {
            return None
        }
        for parent in &self.parents {
            if !env.analysis
                .objects
                .contains_key(&parent.type_id.full_name(&env.library))
            {
                continue;
            }

            let o = &env.analysis.objects[&parent.type_id.full_name(&env.library)];
            if !o.is_interface{
                return Some(o);
            }
        }
        None
    }

}

pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
    try!(general::start_comments(w, &env.config));

    try!(statics_ffi::after_extern_crates(w));
    try!(statics_ffi::use_glib(w));
    try!(statics::include_custom_modules(w, env));
    try!(statics::use_subclass_modules(w, env));
    try!(general::uses(w, env, &analysis.imports));


    // match &*env.config.library_name {
    //     "GLib" => try!(statics::only_for_glib(w)),
    //     "GObject" => try!(statics::only_for_gobject(w)),
    //     "Gtk" => try!(statics::only_for_gtk(w)),
    //     _ => (),
    // }
    try!(writeln!(w));


    // TODO: insert gobject-subclass uses

    let subclass_info = SubclassInfo::new(env, analysis);

    try!(generate_impl(w, env, analysis, &subclass_info));

    if !analysis.is_interface{
        try!(generate_impl_ext(w, env, analysis, &subclass_info));
    }

    try!(generate_any_impl(w, env, analysis, &subclass_info));

    if !analysis.is_interface{
        try!(generate_base(w, env, analysis, &subclass_info));
        try!(generate_ext(w, env, analysis, &subclass_info));
        try!(generate_glib_wrapper(w, env, analysis, &subclass_info));
        try!(generate_impl_base(w, env, analysis, &subclass_info));
        try!(generate_class(w, env, analysis, &subclass_info));
        try!(generate_parent_impls(w, env, analysis, &subclass_info));
        try!(generate_interface_impls(w, env, analysis, &subclass_info));
        try!(generate_box_impl(w, env, analysis, &subclass_info));
        try!(generate_impl_objecttype(w, env, analysis, &subclass_info));
    }else{
        try!(generate_impl_static(w, env, analysis, &subclass_info));
    }

    try!(generate_extern_c_funcs(w, env, analysis, &subclass_info));

    Ok(())
}

pub fn generate_exports(
    env: &Env,
    analysis: &analysis::object::Info,
    module_name: &str,
    contents: &mut Vec<String>,
) {
    let cfg_condition = general::cfg_condition_string(&analysis.cfg_condition, false, 0);
    let version_cfg = general::version_condition_string(env, analysis.version, false, 0);
    let mut cfg = String::new();
    if let Some(s) = cfg_condition {
        cfg.push_str(&s);
        cfg.push('\n');
    };
    if let Some(s) = version_cfg {
        cfg.push_str(&s);
        cfg.push('\n');
    };
    contents.push("".to_owned());
    contents.push(format!("{}pub mod {};", cfg, module_name));
    // contents.push(format!(
    //     "{}pub use self::{}::{};",
    //     cfg,
    //     module_name,
    //     analysis.name
    // ));
}

pub fn generate_impl(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {

    // start impl trait
    try!(writeln!(w));

    if object_analysis.is_interface{

        // TODO: Can I use a generic parent 'T' here, too? That'd be easier
        try!(writeln!(
            w,
            "pub trait {}: AnyImpl + 'static {{",
            object_analysis.subclass_impl_trait_name
        ));
    }else{

        let parents = subclass_info.parent_names(env, "_subclass");

        let parent_impls: Vec<String> = parents
            .iter()
            .map(|ref p| format!(" {}Impl<T> +", p))
            .collect();
        let parent_objs = parent_impls.join("");

        try!(writeln!(
            w,
            "pub trait {}<T: {}>:{} ObjectImpl<T> + AnyImpl + 'static {{",
            object_analysis.subclass_impl_trait_name,
            object_analysis.subclass_base_trait_name,
            parent_objs
        ));

        info!("supertypes, {:?}", parents);
    }


    for method_analysis in &object_analysis.virtual_methods {

        if object_analysis.is_interface{
            try!(virtual_methods::generate_declaration(
                w,
                env,
                object_analysis,
                method_analysis,
                subclass_info,
                1
            ));
        }else{

            try!(virtual_methods::generate_default_impl(
                w,
                env,
                object_analysis,
                method_analysis,
                subclass_info,
                1
            ));
        }
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
    try!(writeln!(w, "pub trait {}<T> {{}}", implext_name));

    // start ext trait impl
    let parents = subclass_info.parent_names(env, "");

    let parent_impls: Vec<String> = parents
        .iter()
        .map(|ref p| format!("+ glib::IsA<{}>", p))
        .collect();
    let parent_objs = parent_impls.join(" ");

    try!(writeln!(
        w,
        "impl<S: {impl_name}<T>, T: ObjectType {parents}> {implext_name}<T> for S {{}}",
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
        object_analysis.subclass_base_trait_name, parent_objs
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

fn generate_any_impl(
    w: &mut Write,
    _env: &Env,
    object_analysis: &analysis::object::Info,
    _subclass_info: &SubclassInfo,
) -> Result<()> {

    try!(writeln!(w));


    if object_analysis.is_interface{
        try!(writeln!(
            w,
            "any_impl!({});",
            object_analysis.subclass_impl_trait_name
        ));
    }else{
        try!(writeln!(
            w,
            "any_impl!({}, {});",
            object_analysis.subclass_base_trait_name, object_analysis.subclass_impl_trait_name
        ));
    }


    Ok(())
}

fn generate_ext(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {
    if object_analysis.class_type.is_none() {
        return Ok(());
    }

    let ext_name = if object_analysis.is_interface{
        format!("{}InterfaceExt", object_analysis.name)
    }else{
        format!("{}Ext", object_analysis.class_type.as_ref().unwrap())
    };



        // start base trait
        try!(writeln!(w));
        try!(writeln!(
            w,
            "pub unsafe trait {}<T: {}>\nwhere\n{}T::ImplType: {}<T>{{",
            ext_name,
            object_analysis.subclass_base_trait_name,
            tabs(1),
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

fn generate_glib_wrapper(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {
    // start base trait
    try!(writeln!(w));
    try!(writeln!(w, "glib_wrapper! {{"));

    try!(writeln!(w));
    try!(write!(
        w,
        "{tabs1}pub struct {obj}(Object<InstanceStruct<{obj}>>)",
        tabs1 = tabs(1),
        obj = object_analysis.name
    ));

    if subclass_info.parents.len() > 0 {
        try!(write!(w, ":["));
        for parent in &subclass_info.parents {
            let t = env.library.type_(parent.type_id);
            let k = &env.namespaces[parent.type_id.ns_id].crate_name;
            try!(write!(
                w,
                "\n{tabs} {krate}::{ty} => {krate}_ffi::{cty}",
                tabs = tabs(2),
                krate = k,
                ty = t.get_name(),
                cty = t.get_glib_name().unwrap()
            ));
        }

        try!(write!(w, "]"));
    }

    try!(writeln!(w, "{tabs1};", tabs1 = tabs(1)));
    try!(writeln!(
        w,
        "{tabs1}match fn {{ \n \
         {tabs2}get_type => || get_type::<{obj}>(),\n \
         {tabs1}}}",
        tabs1 = tabs(1),
        tabs2 = tabs(2),
        obj = object_analysis.name
    ));

    try!(writeln!(w));
    try!(writeln!(w, "}}"));

    Ok(())
}

fn generate_impl_base(
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

    try!(writeln!(w));
    try!(writeln!(
        w,
        "unsafe impl<T: ObjectType {}> {} for T {{}}",
        parent_objs, object_analysis.subclass_base_trait_name
    ));

    Ok(())
}

fn generate_class(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {
    try!(writeln!(w));

    writeln!(
        w,
        "pub type {obj}Class = ClassStruct<{obj}>;",
        obj = object_analysis.name
    );

    Ok(())
}

fn generate_parent_impls(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {
    try!(writeln!(w));

    writeln!(w, "// FIXME: Boilerplate");
    if subclass_info.parents.len() > 0 {
        for parent in &subclass_info.parents {
            let t = env.library.type_(parent.type_id);
            try!(writeln!(
                w,
                "unsafe impl {par}ClassExt<{obj}> for {obj}Class {{}}",
                obj = object_analysis.name,
                par = t.get_name()
            ));
        }
    }

    Ok(())
}

fn generate_interface_impls(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {
    try!(writeln!(w));

    writeln!(w, "// FIXME: Boilerplate");
    if subclass_info.parents.len() > 0 {
        for parent in &subclass_info.parents {
            let t = env.library.type_(parent.type_id);
            try!(writeln!(
                w,
                "unsafe impl {par}ClassExt<{obj}> for {obj}Class {{}}",
                obj = object_analysis.name,
                par = t.get_name()
            ));
        }
    }

    Ok(())
}

fn generate_box_impl(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {
    try!(writeln!(w));

    try!(writeln!(w, "#[macro_export]"));
    try!(writeln!(
        w,
        "macro_rules! box_{}_impl(",
        object_analysis.name.to_lowercase()
    ));

    try!(writeln!(w, "{}($name:ident) => {{", tabs(1)));

    if subclass_info.parents.len() > 0 {
        for parent in &subclass_info.parents {
            if !env.analysis
                .objects
                .contains_key(&parent.type_id.full_name(&env.library))
            {
                continue;
            }
            let o = &env.analysis.objects[&parent.type_id.full_name(&env.library)];
            try!(writeln!(
                w,
                "{}box_{}_impl!($name);",
                tabs(2),
                o.name.to_lowercase()
            ));
        }
    } else {
        try!(writeln!(w, "{}box_object_impl!($name);", tabs(2)));
    }

    let obj = &env.config.objects[&object_analysis.full_name];
    let mod_name = obj.module_name.clone().unwrap_or_else(|| {
        nameutil::module_name(nameutil::split_namespace_name(&object_analysis.full_name).1)
    });

    try!(writeln!(w, "{tabs}impl<T: $crate::{mo}::{base}> $crate::{mo}::{impl}<T> for Box<$name<T>>{{",
                  tabs=tabs(2),
                  mo=mod_name,
                  base=object_analysis.subclass_base_trait_name,
                  impl=object_analysis.subclass_impl_trait_name));


    for method_analysis in &object_analysis.virtual_methods {
        try!(virtual_methods::generate_box_impl(
            w,
            env,
            object_analysis,
            method_analysis,
            subclass_info,
            3
        ));
    }

    try!(writeln!(w, "{}}}", tabs(2)));

    try!(writeln!(w, "{}}}", tabs(1)));
    try!(writeln!(w, ");"));

    Ok(())
}

fn generate_impl_objecttype(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {


    try!(writeln!(w));

    try!(writeln!(w, "impl ObjectType for {}{{",
        object_analysis.name
    ));

    try!(writeln!(w, "{}const NAME: &'static str = \"Rs{}\";",
        tabs(1),
        object_analysis.full_name
    ));

    let parent = subclass_info.parent(env);

    if parent.is_some(){

        let p = parent.as_ref().unwrap();

        let (ns, n) = nameutil::split_namespace_name(&p.full_name);

        try!(writeln!(w, "{}type ParentType = {}::{};",
            tabs(1),
            ns.unwrap_or("").to_lowercase(),
            n
        ));
    }

    try!(writeln!(w, "{}type ImplType = Box<{}<Self>>;",
        tabs(1),
        object_analysis.subclass_impl_trait_name
    ));

    try!(writeln!(w, "{}type InstanceStructType = InstanceStruct<Self>;",
        tabs(1)
    ));

    try!(writeln!(w, "{}fn class_init(token: &ClassInitToken, klass: &mut {}Class) {{",
        tabs(1),
        object_analysis.name
    ));

    try!(writeln!(w, "{}ObjectClassExt::override_vfuncs(klass, token);",
        tabs(2)
    ));


    for parent in &subclass_info.parents {
        try!(writeln!(w, "{}{}ClassExt::override_vfuncs(klass, token);",
                      tabs(2),
                      parent.name));
    }

    try!(writeln!(w, "{}}}", tabs(1)));


    try!(writeln!(w, "{}object_type_fns!();",
        tabs(1)
    ));

    try!(writeln!(w, "}}"));


    Ok(())
}

fn generate_impl_static(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {
    try!(writeln!(w));

    writeln!(
        w,
        "pub trait {}Static<T: ObjectType>: 'static {{",
        object_analysis.subclass_impl_trait_name
    );

    try!(writeln!(w, "{}fn get_impl<'a>(&self, imp: &'a T::ImplType) -> &'a {};",
        tabs(1),
        object_analysis.subclass_impl_trait_name
    ));


    // TODO: What other functions are needed here??


    writeln!(
        w,
        "}}"
    );

    try!(writeln!(w));


    writeln!(
        w,
        "struct {}Static<T: ObjectType>{{",
        object_analysis.name
    );


    try!(writeln!(w, "{}imp_static: *const {}Static<T>",
        tabs(1),
        object_analysis.subclass_impl_trait_name
    ));

    writeln!(
        w,
        "}}"
    );


    Ok(())
}


fn generate_extern_c_funcs(
    w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {


    for method_analysis in &object_analysis.virtual_methods {
        try!(virtual_methods::generate_extern_c_func(
            w,
            env,
            object_analysis,
            method_analysis,
            subclass_info,
            0
        ));
    }

    if object_analysis.is_interface{

        try!(virtual_methods::generate_interface_init(
            w,
            env,
            object_analysis,
            subclass_info,
            0
        ));

        try!(virtual_methods::generate_interface_get_type(
            w,
            env,
            object_analysis,
            subclass_info,
            0
        ));

        // TODO: generate register_*<T: ObjectType, I: *ImplStatic<T>>(
        // see: https://github.com/sdroege/gst-plugin-rs/blob/25af5afb2bb9dfea79a13fd306d4b7fe36d26496/gst-plugin/src/uri_handler.rs#L123
    }



    Ok(())
}
