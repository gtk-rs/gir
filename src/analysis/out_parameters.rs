use std::slice::Iter;
use std::vec::Vec;

use analysis::imports::Imports;
use analysis::ref_mode::RefMode;
use env::Env;
use library::*;
use super::conversion_type::ConversionType;
use super::rust_type::parameter_rust_type;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    None,
    Normal,
    Optional,
    Combined,
}

impl Default for Mode {
    fn default() -> Mode {
        Mode::None
    }
}

#[derive(Debug, Default)]
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

pub fn analyze(env: &Env, func: &Function) -> (Info, bool) {
    let mut info: Info = Default::default();
    let mut unsupported_outs = false;

    if func.throws {
        //TODO: throwable functions
        return (info, true);
    } else if func.ret.typ == TypeId::tid_none() {
        info.mode = Mode::Normal;
    } else if func.ret.typ == TypeId::tid_bool() {
        info.mode = Mode::Optional;
    } else {
        info.mode = Mode::Combined;
    }

    for par in &func.parameters {
        if par.direction != ParameterDirection::Out { continue; }
        if can_as_return(env, par) {
            info.params.push(par.clone());
        } else {
            unsupported_outs = true;
        }
    }

    if info.params.is_empty() {
        info.mode = Mode::None;
    }
    if info.mode == Mode::Combined {
        info.params.insert(0, func.ret.clone());
    }

    (info, unsupported_outs)
}

pub fn analyze_imports(env: &Env, func: &Function, imports: &mut Imports) {
    for par in &func.parameters {
        if par.direction == ParameterDirection::Out && !par.caller_allocates {
            match *env.library.type_(par.typ) {
                Type::Fundamental(..) |
                    Type::Bitfield(..) |
                    Type::Enumeration(..) => imports.add("std::mem".into(), func.version),
                _ => imports.add("std::ptr".into(), func.version),
            }
        }
    }
}

fn can_as_return(env: &Env, par: &Parameter) -> bool {
    use super::conversion_type::ConversionType::*;
    match ConversionType::of(&env.library, par.typ) {
        Direct => true,
        Scalar => true,
        Pointer => parameter_rust_type(env, par.typ, ParameterDirection::Out, Nullable(false), RefMode::None).is_ok(),
        Unknown => false,
    }
}
