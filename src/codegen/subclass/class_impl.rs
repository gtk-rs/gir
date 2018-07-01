use std::io::{Result, Write};
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;

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
            let obj = &env.config.objects[&o.full_name];

            if obj.status.ignored() {
                continue;
            }

            if !o.is_interface{
                return Some(o);
            }
        }
        None
    }

    fn get_all_parents<'a>(&self, env: &'a Env) -> Vec<&'a analysis::object::Info>
    {
        let mut pars = Vec::new();
        for parent in &self.parents {
            if !env.analysis
                .objects
                .contains_key(&parent.type_id.full_name(&env.library))
            {
                continue;
            }

            let o = &env.analysis.objects[&parent.type_id.full_name(&env.library)];
            let obj = &env.config.objects[&o.full_name];
            debug!("object status {:?}: {:?}", o.full_name, obj.status);
            if obj.status.ignored() {
                continue;
            }
            pars.push(o);
        }
        pars
    }

    fn get_parents<'a>(&self, env: &'a Env) -> Vec<&'a analysis::object::Info>
    {
        let mut pars = Vec::new();
        for parent in &self.parents {
            if !env.analysis
                .objects
                .contains_key(&parent.type_id.full_name(&env.library))
            {
                continue;
            }

            let o = &env.analysis.objects[&parent.type_id.full_name(&env.library)];
            let obj = &env.config.objects[&o.full_name];
            debug!("object status {:?}: {:?}", o.full_name, obj.status);

            if obj.status.ignored() {
                continue;
            }
            if !o.is_interface{
                pars.push(o);
            }
        }
        pars
    }

    fn get_interfaces<'a>(&self, env: &'a Env) -> Vec<&'a analysis::object::Info>
    {
        let mut ifaces = Vec::new();
        for parent in &self.parents {
            if !env.analysis
                .objects
                .contains_key(&parent.type_id.full_name(&env.library))
            {
                continue;
            }
            let o = &env.analysis.objects[&parent.type_id.full_name(&env.library)];
            let obj = &env.config.objects[&o.full_name];
            debug!("object status {:?}: {:?}", o.full_name, obj.status);

            if obj.status.ignored() {
                continue;
            }

            if o.is_interface{
                ifaces.push(o);
            }
        }
        ifaces
    }

}

fn insert_from_file(w: &mut Write, path: &PathBuf) -> bool{
    if let Ok(mut file) = File::open(&path) {
        let mut custom_str = String::new();
        file.read_to_string(&mut custom_str).unwrap();
        write!(w, "{}", custom_str);
        return true;
    }
    false
}

pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::object::Info) -> Result<()> {
    try!(general::start_comments(w, &env.config));

    try!(statics_ffi::after_extern_crates(w));
    try!(statics::use_glib(w));
    try!(statics::include_custom_modules(w, env));
    try!(general::uses(w, env, &analysis.imports));

    let subclass_info = SubclassInfo::new(env, analysis);
    try!(statics::use_subclass_modules(w, env));
    try!(generate_subclass_uses(w, env, analysis, &subclass_info));


    try!(writeln!(w));

    let path = env.config.target_path.join("src").join("custom")
                                     .join(format!("{}-main.rs",
                                           analysis.module_name(env).unwrap_or(analysis.name.to_lowercase())));

    insert_from_file(w, &path);

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
        try!(generate_box_impl(w, env, analysis, &subclass_info));
        try!(generate_impl_objecttype(w, env, analysis, &subclass_info));
    }else{
        try!(generate_impl_static(w, env, analysis, &subclass_info));
        try!(generate_interface_impls(w, env, analysis, &subclass_info));
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

fn subclass_parent_module_path(parent: &analysis::object::Info, object: &analysis::object::Info, env: &Env, for_use: bool) -> String{

    let mut ns = "".to_string();
    let mut alias = "".to_string();
    let obj = &env.config.objects[&parent.full_name];
    let module_name = obj.module_name.clone().unwrap_or_else(|| {
        nameutil::module_name(nameutil::split_namespace_name(&parent.full_name).1)
    });

    if parent.type_id.ns_id != object.type_id.ns_id{
        let ns_name = &env.library.namespace(parent.type_id.ns_id).name;
        alias = format!("{}_{}", ns_name.to_lowercase(), module_name).to_string();

        if for_use{
            ns = format!("{}_subclass::", ns_name.to_lowercase()).to_string();
        }
    }

    if for_use{
        format!("{}{} as {}", ns, module_name, alias)
    }else{
        format!("{}", alias)
    }
}

pub fn generate_subclass_uses(w: &mut Write,
    env: &Env,
    object_analysis: &analysis::object::Info,
    subclass_info: &SubclassInfo,
) -> Result<()> {

    for parent in &subclass_info.get_all_parents(env){

        let parent_module_path = subclass_parent_module_path(parent, object_analysis, env, true);

        try!(writeln!(
            w,
            "use {};",
            parent_module_path
        ));
    }

    Ok(())
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

        let parent_impls: Vec<String> = subclass_info.get_parents(env)
            .iter()
            .map(|ref p| {
                let templ = if p.is_interface { "" } else {"<T>"};
                format!(" {}::{}Impl{} +", subclass_parent_module_path(p, object_analysis, env, false), p.name, templ) })
            .collect();

        let parent_objs = parent_impls.join("");

        try!(writeln!(
            w,
            "pub trait {}<T: {}>:{} ObjectImpl<T> + AnyImpl + 'static {{",
            object_analysis.subclass_impl_trait_name,
            object_analysis.subclass_base_trait_name,
            parent_objs
        ));
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

            let path = env.config.target_path.join("src").join("custom")
                                             .join(format!("{}-{}-impl.rs",
                                                   object_analysis.module_name(env).unwrap_or(object_analysis.name.to_lowercase()),
                                                   method_analysis.name.to_lowercase()));

            if !insert_from_file(w, &path) {
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
    let mut impls: Vec<&analysis::object::Info> = vec![object_analysis];
    impls.append(&mut subclass_info.get_parents(env));

    let parent_impls: Vec<String> = impls.iter()
                                .map(|ref p| {
                                    let ns_name = &env.namespaces[p.type_id.ns_id].crate_name;
                                    format!("+ glib::IsA<{}::{}>", ns_name, p.name)
                                })
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

    let mut impls: Vec<&analysis::object::Info> = vec![object_analysis];
    impls.append(&mut subclass_info.get_parents(env));

    let parent_impls: Vec<String> = impls.iter()
                                .map(|ref p| {
                                    let ns_name = &env.namespaces[p.type_id.ns_id].crate_name;
                                    format!("+ glib::IsA<{}::{}>", ns_name, p.name)
                                })
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

        let path = env.config.target_path.join("src").join("custom")
                                         .join(format!("{}-{}-base.rs",
                                               object_analysis.module_name(env).unwrap_or(object_analysis.name.to_lowercase()),
                                               method_analysis.name.to_lowercase()));

        if !insert_from_file(w, &path) {
            try!(virtual_methods::generate_base_impl(
                w,
                env,
                object_analysis,
                method_analysis,
                subclass_info,
                1
            ));
        }
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

        let t = env.library.type_(object_analysis.type_id);
        let k = &env.namespaces[object_analysis.type_id.ns_id].crate_name;
        try!(write!(
            w,
            "\n{tabs} {krate}::{ty} => {krate}_ffi::{cty},",
            tabs = tabs(2),
            krate = k,
            ty = t.get_name(),
            cty = t.get_glib_name().unwrap()
        ));

        for parent in &subclass_info.parents {
            let t = env.library.type_(parent.type_id);
            let k = &env.namespaces[parent.type_id.ns_id].crate_name;

            if t.get_name() == "InitiallyUnowned"{
                continue;
            }

            try!(write!(
                w,
                "\n{tabs} {krate}::{ty} => {krate}_ffi::{cty},",
                tabs = tabs(2),
                krate = k,
                ty = t.get_name(),
                cty = t.get_glib_name().unwrap_or("")
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

    let mut impls: Vec<&analysis::object::Info> = vec![object_analysis];
    impls.append(&mut subclass_info.get_parents(env));

    let parent_impls: Vec<String> = impls.iter()
                                .map(|ref p| {
                                    let ns_name = &env.namespaces[p.type_id.ns_id].crate_name;
                                    format!("+ glib::IsA<{}::{}>", ns_name, p.name)
                                })
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

    let parents = subclass_info.get_parents(env);
    writeln!(w, "// FIXME: Boilerplate");

    try!(writeln!(
        w,
        "unsafe impl {par}ClassExt<{obj}> for {obj}Class {{}}",
        obj = object_analysis.name,
        par = "Object".to_owned()
    ));

    try!(writeln!(
        w,
        "unsafe impl {par}ClassExt<{obj}> for {obj}Class {{}}",
        obj = object_analysis.name,
        par = object_analysis.name
    ));

    for parent in parents {
        try!(writeln!(
            w,
            "unsafe impl {par_mod}::{par}ClassExt<{obj}> for {obj}Class {{}}",
            obj = object_analysis.name,
            par_mod = subclass_parent_module_path(parent, object_analysis, env, false),
            par = parent.name
        ));
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

fn box_impl_name(env: &Env,
                 analysis: &analysis::object::Info) -> String{
    format!("box_{}_{}_impl",
        env.namespaces[analysis.type_id.ns_id].name.to_lowercase(),
        analysis.module_name(env).unwrap_or(analysis.name.to_lowercase()))
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
        "macro_rules! {}(",
        box_impl_name(env, object_analysis)
    ));

    try!(writeln!(w, "{}($name:ident) => {{", tabs(1)));

    let parents = subclass_info.get_parents(env);
    if parents.len() > 0 {
        for parent in parents {
            try!(writeln!(
                w,
                "{}{}!($name);",
                tabs(2),
                box_impl_name(env, &parent)
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

        let path = env.config.target_path.join("src").join("custom")
                                         .join(format!("{}-{}-box_impl.rs",
                                               object_analysis.module_name(env).unwrap_or(object_analysis.name.to_lowercase()),
                                               method_analysis.name.to_lowercase()));

        if !insert_from_file(w, &path) {
            try!(virtual_methods::generate_box_impl(
                w,
                env,
                object_analysis,
                method_analysis,
                subclass_info,
                3
            ));
        }
    }

    try!(writeln!(w, "{}}}", tabs(2)));

    try!(writeln!(w, "{}}}", tabs(1)));
    try!(writeln!(w, ");"));

    try!(writeln!(w));

    try!(writeln!(w, "{}!({});", box_impl_name(env, object_analysis), object_analysis.subclass_impl_trait_name));

    Ok(())
}


fn override_vfuncs_statement(name: &String) -> String{
    format!("{}ClassExt::override_vfuncs(klass, token);", name)
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
        object_analysis.full_name.replace(".", "")
    ));


    let (ns, n) = nameutil::split_namespace_name(&object_analysis.full_name);
    try!(writeln!(w, "{}type ParentType = {}::{};",
        tabs(1),
        ns.unwrap_or("").to_lowercase(),
        n
    ));


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

    try!(writeln!(w, "{}{}", tabs(2), override_vfuncs_statement(&"Object".to_string())));
    try!(writeln!(w, "{}{}", tabs(2), override_vfuncs_statement(&object_analysis.name)));
    for parent in &subclass_info.get_parents(env) {
        let par_mod = subclass_parent_module_path(parent, object_analysis, env, false);
        try!(writeln!(w, "{}{}::{}", tabs(2), par_mod, override_vfuncs_statement(&parent.name)));
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

        let path = env.config.target_path.join("src").join("custom")
                                         .join(format!("{}-{}-trampoline.rs",
                                               object_analysis.module_name(env).unwrap_or(object_analysis.name.to_lowercase()),
                                               method_analysis.name.to_lowercase()));

        if !insert_from_file(w, &path) {
            try!(virtual_methods::generate_extern_c_func(
                w,
                env,
                object_analysis,
                method_analysis,
                subclass_info,
                0
            ));
        }
    }

    if object_analysis.is_interface{

        try!(virtual_methods::generate_interface_init(
            w,
            env,
            object_analysis,
            subclass_info,
            0
        ));

        try!(virtual_methods::generate_interface_register(
            w,
            env,
            object_analysis,
            subclass_info,
            0
        ));

    }



    Ok(())
}
