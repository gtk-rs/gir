use std::collections::BTreeMap;

use env::Env;
use library::{Type, TypeId};

pub mod bounds;
pub mod c_type;
pub mod child_properties;
pub mod class_hierarchy;
pub mod conversion_type;
pub mod ffi_type;
pub mod functions;
pub mod general;
pub mod imports;
pub mod info_base;
pub mod namespaces;
pub mod object;
pub mod out_parameters;
pub mod parameter;
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

#[derive(Default)]
pub struct Analysis {
    pub objects: BTreeMap<String, object::Info>,
    pub records: BTreeMap<String, record::Info>,
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
        for &(tid, ref deps) in &to_analyze {
            if !is_all_deps_analyzed(env, &deps) {
                new_to_analyze.push((tid, deps.clone()));
                continue;
            }
            analyze(env, tid, deps);
            analyzed += 1;
        }

        to_analyze = new_to_analyze;
    }
    if !to_analyze.is_empty() {
        error!("Not analyzed {} objects due unfinished dependencies", to_analyze.len());
    }
}

fn analyze(env: &mut Env, tid: TypeId, deps: &[TypeId]) {
    let full_name = tid.full_name(&env.library);
    let obj = match env.config.objects.get(&*full_name) {
        Some(obj) => obj,
        None => return,
    };
    match *env.library.type_(tid) {
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

fn is_all_deps_analyzed(env: &mut Env, deps: &[TypeId]) -> bool
{
    for tid in deps {
        let full_name = tid.full_name(&env.library);
        if !env.analysis.objects.contains_key(&full_name) {
            return false
        }
    }
    true
}
