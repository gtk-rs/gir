use std::borrow::Cow;
use std::collections::HashSet;
use std::vec::Vec;

use analysis::needed_upcast::needed_upcast;
use analysis::out_parameters;
use analysis::return_value;
use analysis::rust_type::*;
use analysis::upcasts::Upcasts;
use env::Env;
use library::{self, Nullable};
use nameutil;
use traits::*;
use version::Version;

//TODO: change use Parameter to reference?
pub struct Info {
    pub name: String,
    pub glib_name: String,
    pub kind: library::FunctionKind,
    pub comented: bool,
    pub class_name: Result,
    pub parameters: Vec<library::Parameter>,
    pub ret: return_value::Info,
    pub upcasts: Upcasts,
    pub outs: out_parameters::Info,
    pub version: Option<Version>,
}

pub fn analyze(env: &Env, klass: &library::Class, class_tid: library::TypeId,
    non_nullable_overrides: &[String], used_types: &mut HashSet<String>) -> Vec<Info> {
    let mut funcs = Vec::new();

    for func in &klass.functions {
        let info = analyze_function(env, func, class_tid, non_nullable_overrides, used_types);
        funcs.push(info);
    }

    funcs
}

fn analyze_function(env: &Env, func: &library::Function, class_tid: library::TypeId,
    non_nullable_overrides: &[String], all_used_types: &mut HashSet<String>) -> Info {
    let mut commented = false;
    let mut upcasts: Upcasts = Default::default();
    let mut used_types: Vec<String> = Vec::with_capacity(4);

    let ret = return_value::analyze(env, func, class_tid, non_nullable_overrides, &mut used_types);
    commented |= ret.commented;

    let mut parameters = func.parameters.clone();

    for (pos, par) in parameters.iter_mut().enumerate() {
        assert!(!par.instance_parameter || pos == 0,
            "Wrong instance parameter in {}", func.c_identifier.as_ref().unwrap());
        if let Ok(s) = used_rust_type(env, par.typ) {
            used_types.push(s);
        }
        if !par.instance_parameter {
            if let Cow::Owned(mangled) = nameutil::mangle_keywords(&*par.name) {
                par.name = mangled;
            }
        }
        if !par.instance_parameter && needed_upcast(&env.library, par.typ) {
            let type_name = rust_type(env, par.typ);
            if !upcasts.add_parameter(&par.name, type_name.as_str()) {
                panic!("Too many parameters upcasts for {}", func.c_identifier.as_ref().unwrap())
            }
        }
        if parameter_rust_type(env, par.typ, par.direction, Nullable(false)).is_err() {
            commented = true;
        }
    }

    if !commented {
        for s in used_types {
            if let Some(i) = s.find("::") {
                all_used_types.insert(s[..i].into());
            }
            else {
                all_used_types.insert(s);
            }
        }
    }

    let outs = out_parameters::analyze(env, func);

    Info {
        name: nameutil::mangle_keywords(&*func.name).into_owned(),
        glib_name: func.c_identifier.as_ref().unwrap().clone(),
        kind: func.kind,
        comented: commented,
        class_name: rust_type(env, class_tid),
        parameters: parameters,
        ret: ret,
        upcasts: upcasts,
        outs: outs,
        version: func.version,
    }
}
