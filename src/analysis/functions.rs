use std::vec::Vec;

use analysis::rust_type::ToRustType;
use env::Env;
use library;

pub struct Info {
    pub name: String,
    pub glib_name: String,
    pub kind: library::FunctionKind,
    pub comented: bool,
    pub class_name: String,
    //TODO: parameters
    pub ret: library::Parameter,
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
    let klass = env.library.type_(class_tid);

    Info {
        name: type_.name.clone(),
        glib_name: type_.c_identifier.clone(),
        kind: type_.kind,
        comented: false,
        class_name: klass.to_rust_type(),
        ret: type_.ret.clone(),
    }
}
