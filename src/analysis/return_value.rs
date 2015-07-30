use std::collections::HashSet;

use analysis::rust_type::*;
use env::Env;
use library;

pub struct Info {
    pub parameter: Option<library::Parameter>,
    pub commented: bool,
}

pub fn analyze(env: &Env, type_: &library::Function,
    used_types: &mut HashSet<String>) -> Info {

    let parameter = if type_.ret.typ == Default::default() { None } else {
        used_rust_type(env, type_.ret.typ).ok().map(|s| used_types.insert(s));
        Some(type_.ret.clone())
    };
    let commented = if type_.ret.typ == Default::default() { false } else {
        parameter_rust_type(env, type_.ret.typ, type_.ret.direction).is_err()
    };

    Info {
        parameter: parameter,
        commented: commented,
    }
}
