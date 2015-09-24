use std::vec::Vec;

use env::Env;
use library::*;
use super::type_kind::TypeKind;

pub type Info = Vec<Parameter>;

pub fn analyze(env: &Env, type_: &Function) -> Info {
    let mut outs = Info::new();
    //Only process out parameters if function returns None
    if type_.ret.typ != Default::default() { return outs; }

    for par in &type_.parameters {
        if can_as_return(env, par) {
            outs.push(par.clone());
        }
    }
    outs
}

fn can_as_return(env: &Env, par: &Parameter) -> bool {
    use super::type_kind::TypeKind::*;
    if par.direction != ParameterDirection::Out { return false; }
    match TypeKind::of(&env.library, par.typ) {
        Enumeration |
            Direct => true,
        _ => false
    }
}
