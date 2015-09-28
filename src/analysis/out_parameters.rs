use std::slice::Iter;
use std::vec::Vec;

use env::Env;
use library::*;
use super::type_kind::TypeKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    None,
    Normal,
}

pub struct Info {
    pub mode: Mode,
    pub params: Vec<Parameter>,
}

impl Info {
    pub fn is_empty(&self) -> bool {
        self.mode == Mode::None
    }

    pub fn iter(&self) -> Iter<Parameter> {
        self.params.iter()
    }

    pub fn len(&self) -> usize {
        self.params.len()
    }
}

pub fn analyze(env: &Env, type_: &Function) -> (Info, bool) {
    let mut info = Info { mode: Mode::None, params: Vec::new() };
    let mut unsupported_outs = false;

    //Only process out parameters if function returns None
    if type_.ret.typ == Default::default() {
        info.mode = Mode::Normal;
    } else {
        return (info, false);
    }

    for par in &type_.parameters {
        if par.direction != ParameterDirection::Out { continue; }
        if can_as_return(env, par) {
            info.params.push(par.clone());
        } else {
            unsupported_outs = true;
        }
    }

    if info.params.is_empty() { info.mode = Mode::None }

    (info, unsupported_outs)
}

fn can_as_return(env: &Env, par: &Parameter) -> bool {
    use super::type_kind::TypeKind::*;
    match TypeKind::of(&env.library, par.typ) {
        Enumeration |
            Converted |
            Direct => true,
        _ => false
    }
}
