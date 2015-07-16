use std::collections::HashSet;
use std::vec::Vec;

use analysis::needed_upcast::needed_upcast;
use analysis::out_parameters;
use analysis::rust_type::*;
use analysis::upcasts::Upcasts;
use env::Env;
use library;
use traits::*;

//TODO: change use Parameter to reference?
pub struct Info {
    pub name: String,
    pub glib_name: String,
    pub kind: library::FunctionKind,
    pub comented: bool,
    pub class_name: Result,
    pub parameters: Vec<library::Parameter>,
    pub ret: Option<library::Parameter>,
    pub upcasts: Upcasts,
    pub outs: out_parameters::Info,
}

pub fn analyze(env: &Env, type_: &library::Class, class_tid: library::TypeId,
    used_types: &mut HashSet<String>) -> Vec<Info> {
    let mut funcs = Vec::new();

    for func in &type_.functions {
        let info = analyze_function(env, func, class_tid, used_types);
        funcs.push(info);
    }

    funcs
}

fn analyze_function(env: &Env, type_: &library::Function, class_tid: library::TypeId,
    used_types: &mut HashSet<String>) -> Info {
    let mut commented = false;
    let mut upcasts: Upcasts = Default::default();

    let ret = if type_.ret.typ == Default::default() { None } else {
        used_rust_type(env, type_.ret.typ).ok().map(|s| used_types.insert(s));
        Some(type_.ret.clone())
    };

    if let Some(ref ret_) = ret {
        if parameter_rust_type(env, ret_.typ, ret_.direction)
            .is_err() { commented = true; }
    }

    for (pos, par) in type_.parameters.iter().enumerate() {
        assert!(!par.instance_parameter || pos == 0,
            "Wrong instance parameter in {}", type_.c_identifier.as_ref().unwrap());
        used_rust_type(env, par.typ).ok().map(|s| used_types.insert(s));
        if !par.instance_parameter && needed_upcast(&env.library, par.typ) {
            let type_name = rust_type(env, par.typ);
            if !upcasts.add_parameter(&par.name, type_name.as_str()) {
                panic!("Too many parameters upcasts for {}", type_.c_identifier.as_ref().unwrap())
            }
        }
        if parameter_rust_type(env, par.typ, par.direction)
            .is_err() { commented = true; }
    }

    let outs = out_parameters::analyze(type_);

    Info {
        name: type_.name.clone(),
        glib_name: type_.c_identifier.as_ref().unwrap().clone(),
        kind: type_.kind,
        comented: commented,
        class_name: rust_type(env, class_tid),
        parameters: type_.parameters.clone(),
        ret: ret,
        upcasts: upcasts,
        outs: outs,
    }
}
