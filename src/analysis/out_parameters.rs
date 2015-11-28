use std::slice::Iter;
use std::vec::Vec;

use env::Env;
use library::*;
use super::conversion_type::ConversionType;
use super::rust_type::parameter_rust_type;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    None,
    Normal,
    Optional,
}

impl Default for Mode {
    fn default() -> Mode {
        Mode::None
    }
}

#[derive(Default)]
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
    let mut info: Info = Default::default();
    let mut unsupported_outs = false;

    if type_.throws {
        //TODO: throwable functions
        return (info, true);
    } else if type_.ret.typ == TypeId::tid_none() {
        info.mode = Mode::Normal;
    } else if type_.ret.typ == TypeId::tid_bool() {
        info.mode = Mode::Optional;
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
    use super::conversion_type::ConversionType::*;
    match ConversionType::of(&env.library, par.typ) {
        Direct => true,
        Scalar => true,
        Pointer => parameter_rust_type(env, par.typ, ParameterDirection::Out, Nullable(false)).is_ok(),
        Unknown => false,
    }
}
