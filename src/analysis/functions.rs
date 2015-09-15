use std::collections::HashSet;
use std::vec::Vec;

use analysis::needed_upcast::needed_upcast;
use analysis::out_parameters;
use analysis::return_value;
use analysis::rust_type::*;
use analysis::upcasts::Upcasts;
use env::Env;
use library;
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
    used_types: &mut HashSet<String>) -> Vec<Info> {
    let mut funcs = Vec::new();

    for func in &klass.functions {
        let info = analyze_function(env, func, class_tid, used_types);
        funcs.push(info);
    }

    funcs
}

fn analyze_function(env: &Env, func: &library::Function, class_tid: library::TypeId,
    used_types: &mut HashSet<String>) -> Info {
    let mut commented = false;
    let mut upcasts: Upcasts = Default::default();

    let ret = return_value::analyze(env, func, class_tid, used_types);
    commented |= ret.commented;

    for (pos, par) in func.parameters.iter().enumerate() {
        assert!(!par.instance_parameter || pos == 0,
            "Wrong instance parameter in {}", func.c_identifier.as_ref().unwrap());
        used_rust_type(env, par.typ).ok().map(|s| used_types.insert(s));
        if !par.instance_parameter && needed_upcast(&env.library, par.typ) {
            let type_name = rust_type(env, par.typ);
            if !upcasts.add_parameter(&par.name, type_name.as_str()) {
                panic!("Too many parameters upcasts for {}", func.c_identifier.as_ref().unwrap())
            }
        }
        if parameter_rust_type(env, par.typ, par.direction)
            .is_err() { commented = true; }
    }

    let outs = out_parameters::analyze(func);

    Info {
        name: func.name.clone(),
        glib_name: func.c_identifier.as_ref().unwrap().clone(),
        kind: func.kind,
        comented: commented,
        class_name: rust_type(env, class_tid),
        parameters: func.parameters.clone(),
        ret: ret,
        upcasts: upcasts,
        outs: outs,
        version: func.version,
    }
}
