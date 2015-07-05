use std::vec::Vec;

use analysis::rust_type::{Result, parameter_rust_type, rust_type};
use env::Env;
use library;

pub struct Info {
    pub name: String,
    pub glib_name: String,
    pub kind: library::FunctionKind,
    pub comented: bool,
    pub class_name: Result,
    pub parameters: Vec<library::Parameter>,
    pub ret: Option<library::Parameter>,
}

pub fn analyze(env: &Env, type_: &library::Class, class_tid: library::TypeId) -> Vec<Info> {
    let mut funcs = Vec::new();

    for func in &type_.functions {
        let info = analyze_function(env, func, class_tid);
        funcs.push(info);
    }

    funcs
}

fn analyze_function(env: &Env, type_: &library::Function, class_tid: library::TypeId) -> Info {
    let mut commented = false;

    let ret = if type_.ret.typ == Default::default() { None } else { Some(type_.ret.clone()) };

    if let Some(ref ret_) = ret {
        if parameter_rust_type(&env.library, ret_.typ, ret_.direction)
            .is_err() { commented = true; }
    }

    for (pos, par) in type_.parameters.iter().enumerate() {
        assert!(!par.instance_parameter || pos == 0,
            "Wrong instance parameter in {}", type_.c_identifier);
        if parameter_rust_type(&env.library, par.typ, par.direction)
            .is_err() { commented = true; }
    }

    Info {
        name: type_.name.clone(),
        glib_name: type_.c_identifier.clone(),
        kind: type_.kind,
        comented: commented,
        class_name: rust_type(&env.library, class_tid),
        parameters: type_.parameters.clone(),
        ret: ret,
    }
}
