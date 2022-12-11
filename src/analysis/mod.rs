use std::collections::BTreeMap;

use imports::Imports;
use log::error;

use crate::{
    env::Env,
    library::{self, Type, TypeId},
};

pub mod bounds;
pub mod c_type;
pub mod child_properties;
pub mod class_builder;
pub mod class_hierarchy;
pub mod constants;
pub mod conversion_type;
pub mod enums;
pub mod ffi_type;
pub mod flags;
pub mod function_parameters;
pub use function_parameters::Parameter;
pub mod functions;
pub mod general;
pub mod imports;
pub mod info_base;
pub mod namespaces;
pub mod object;
pub mod out_parameters;
mod override_string_type;
pub mod properties;
pub mod record;
pub mod record_type;
pub mod ref_mode;
pub mod return_value;
pub mod rust_type;
pub mod safety_assertion_mode;
pub mod signals;
pub mod signatures;
pub mod special_functions;
pub mod supertypes;
pub mod symbols;
pub mod trampoline_parameters;
pub mod trampolines;
pub mod try_from_glib;
pub mod types;

#[derive(Debug, Default)]
pub struct Analysis {
    pub objects: BTreeMap<String, object::Info>,
    pub records: BTreeMap<String, record::Info>,
    pub global_functions: Option<info_base::InfoBase>,
    pub constants: Vec<constants::Info>,

    pub enumerations: Vec<enums::Info>,
    pub enum_imports: Imports,

    pub flags: Vec<flags::Info>,
    pub flags_imports: Imports,
}

fn find_function<'a>(
    env: &Env,
    mut functions: impl Iterator<Item = &'a functions::Info>,
    search_fn: impl Fn(&functions::Info) -> bool + Copy,
) -> Option<&'a functions::Info> {
    functions.find(|fn_info| fn_info.should_be_doc_linked(env) && search_fn(fn_info))
}

impl Analysis {
    pub fn find_global_function<F: Fn(&functions::Info) -> bool + Copy>(
        &self,
        env: &Env,
        search: F,
    ) -> Option<&functions::Info> {
        self.global_functions
            .as_ref()
            .and_then(move |info| find_function(env, info.functions.iter(), search))
    }

    pub fn find_record_by_function<
        F: Fn(&functions::Info) -> bool + Copy,
        G: Fn(&record::Info) -> bool + Copy,
    >(
        &self,
        env: &Env,
        search_record: G,
        search_fn: F,
    ) -> Option<(&record::Info, &functions::Info)> {
        self.records
            .values()
            .filter(|r| search_record(r))
            .find_map(|record_info| {
                find_function(env, record_info.functions.iter(), search_fn)
                    .map(|fn_info| (record_info, fn_info))
            })
    }

    pub fn find_object_by_function<
        F: Fn(&functions::Info) -> bool + Copy,
        G: Fn(&object::Info) -> bool + Copy,
    >(
        &self,
        env: &Env,
        search_obj: G,
        search_fn: F,
    ) -> Option<(&object::Info, &functions::Info)> {
        self.objects
            .values()
            .filter(|o| search_obj(o))
            .find_map(|obj_info| {
                find_function(env, obj_info.functions.iter(), search_fn)
                    .map(|fn_info| (obj_info, fn_info))
            })
    }

    pub fn find_enum_by_function<
        F: Fn(&functions::Info) -> bool + Copy,
        G: Fn(&enums::Info) -> bool + Copy,
    >(
        &self,
        env: &Env,
        search_enum: G,
        search_fn: F,
    ) -> Option<(&enums::Info, &functions::Info)> {
        self.enumerations
            .iter()
            .filter(|o| search_enum(o))
            .find_map(|obj_info| {
                find_function(env, obj_info.functions.iter(), search_fn)
                    .map(|fn_info| (obj_info, fn_info))
            })
    }

    pub fn find_flag_by_function<
        F: Fn(&functions::Info) -> bool + Copy,
        G: Fn(&flags::Info) -> bool + Copy,
    >(
        &self,
        env: &Env,
        search_flag: G,
        search_fn: F,
    ) -> Option<(&flags::Info, &functions::Info)> {
        self.flags
            .iter()
            .filter(|o| search_flag(o))
            .find_map(|obj_info| {
                find_function(env, obj_info.functions.iter(), search_fn)
                    .map(|fn_info| (obj_info, fn_info))
            })
    }
}

pub fn run(env: &mut Env) {
    let mut to_analyze: Vec<(TypeId, Vec<TypeId>)> = Vec::with_capacity(env.config.objects.len());
    for obj in env.config.objects.values() {
        if obj.status.ignored() {
            continue;
        }
        let tid = match env.library.find_type(0, &obj.name) {
            Some(x) => x,
            None => continue,
        };
        let deps = supertypes::dependencies(env, tid);
        to_analyze.push((tid, deps));
    }

    let mut analyzed = 1;
    while analyzed > 0 {
        analyzed = 0;
        let mut new_to_analyze: Vec<(TypeId, Vec<TypeId>)> = Vec::with_capacity(to_analyze.len());
        for (tid, ref deps) in to_analyze {
            if !is_all_deps_analyzed(env, deps) {
                new_to_analyze.push((tid, deps.clone()));
                continue;
            }
            analyze(env, tid, deps);
            analyzed += 1;
        }

        to_analyze = new_to_analyze;
    }

    if !to_analyze.is_empty() {
        error!(
            "Not analyzed {} objects due unfinished dependencies",
            to_analyze.len()
        );
        return;
    }

    analyze_enums(env);

    analyze_flags(env);

    analyze_constants(env);

    // Analyze free functions as the last step once all types are analyzed
    analyze_global_functions(env);
}

fn analyze_enums(env: &mut Env) {
    let mut imports = Imports::new(&env.library);

    for obj in env.config.objects.values() {
        if obj.status.ignored() {
            continue;
        }
        let tid = match env.library.find_type(0, &obj.name) {
            Some(x) => x,
            None => continue,
        };

        if let Type::Enumeration(_) = env.library.type_(tid) {
            if let Some(info) = enums::new(env, obj, &mut imports) {
                env.analysis.enumerations.push(info);
            }
        }
    }

    env.analysis.enum_imports = imports;
}

fn analyze_flags(env: &mut Env) {
    let mut imports = Imports::new(&env.library);

    for obj in env.config.objects.values() {
        if obj.status.ignored() {
            continue;
        }
        let tid = match env.library.find_type(0, &obj.name) {
            Some(x) => x,
            None => continue,
        };

        if let Type::Bitfield(_) = env.library.type_(tid) {
            if let Some(info) = flags::new(env, obj, &mut imports) {
                env.analysis.flags.push(info);
            }
        }
    }

    env.analysis.flags_imports = imports;
}

fn analyze_global_functions(env: &mut Env) {
    let ns = env.library.namespace(library::MAIN_NAMESPACE);

    let full_name = format!("{}.*", ns.name);

    let obj = match env.config.objects.get(&*full_name) {
        Some(obj) if obj.status.need_generate() => obj,
        _ => return,
    };

    let functions: Vec<_> = ns
        .functions
        .iter()
        .filter(|f| f.kind == library::FunctionKind::Global)
        .collect();
    if functions.is_empty() {
        return;
    }

    let mut imports = imports::Imports::new(&env.library);
    imports.add("glib::translate::*");

    let functions = functions::analyze(
        env,
        &functions,
        None,
        false,
        false,
        obj,
        &mut imports,
        None,
        None,
    );

    env.analysis.global_functions = Some(info_base::InfoBase {
        full_name,
        type_id: TypeId::tid_none(),
        name: "*".into(),
        functions,
        imports,
        ..Default::default()
    });
}

fn analyze_constants(env: &mut Env) {
    let ns = env.library.namespace(library::MAIN_NAMESPACE);

    let full_name = format!("{}.*", ns.name);

    let obj = match env.config.objects.get(&*full_name) {
        Some(obj) if obj.status.need_generate() => obj,
        _ => return,
    };

    let constants: Vec<_> = ns.constants.iter().collect();
    if constants.is_empty() {
        return;
    }

    env.analysis.constants = constants::analyze(env, &constants, obj);
}

fn analyze(env: &mut Env, tid: TypeId, deps: &[TypeId]) {
    let full_name = tid.full_name(&env.library);
    let obj = match env.config.objects.get(&*full_name) {
        Some(obj) => obj,
        None => return,
    };
    match env.library.type_(tid) {
        Type::Class(_) => {
            if let Some(info) = object::class(env, obj, deps) {
                env.analysis.objects.insert(full_name, info);
            }
        }
        Type::Interface(_) => {
            if let Some(info) = object::interface(env, obj, deps) {
                env.analysis.objects.insert(full_name, info);
            }
        }
        Type::Record(_) => {
            if let Some(info) = record::new(env, obj) {
                env.analysis.records.insert(full_name, info);
            }
        }
        _ => {}
    }
}

fn is_all_deps_analyzed(env: &mut Env, deps: &[TypeId]) -> bool {
    for tid in deps {
        let full_name = tid.full_name(&env.library);
        if !env.analysis.objects.contains_key(&full_name) {
            return false;
        }
    }
    true
}

pub fn is_gpointer(s: &str) -> bool {
    s == "gpointer" || s == "void*"
}
