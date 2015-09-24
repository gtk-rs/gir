use std::vec::Vec;

use env::Env;
use library::*;
use super::type_kind::TypeKind;

pub type Info = Vec<Parameter>;

pub fn analyze(env: &Env, type_: &Function) -> (Info, bool) {
    let mut outs = Info::new();
    let mut unsupported_outs = false;
    //Only process out parameters if function returns None
    if type_.ret.typ != Default::default() { return (outs, false); }

    for par in &type_.parameters {
        if par.direction != ParameterDirection::Out { continue; }
        if can_as_return(env, par) {
            outs.push(par.clone());
        } else {
            unsupported_outs = true;
        }
    }

    (outs, unsupported_outs)
}

fn can_as_return(env: &Env, par: &Parameter) -> bool {
    use super::type_kind::TypeKind::*;
    match TypeKind::of(&env.library, par.typ) {
        Enumeration |
            Direct => true,
        _ => false
    }
}
