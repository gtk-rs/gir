use std::str::FromStr;

use crate::{analysis::function_parameters::Parameters, env::Env, library};

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyAssertionMode {
    #[default]
    None,
    Skip,
    NotInitialized,
    InMainThread,
}

impl FromStr for SafetyAssertionMode {
    type Err = String;
    fn from_str(name: &str) -> Result<SafetyAssertionMode, String> {
        match name {
            "none" => Ok(Self::None),
            "skip" => Ok(Self::Skip),
            "not-initialized" => Ok(Self::NotInitialized),
            "in-main-thread" => Ok(Self::InMainThread),
            _ => Err(format!("Unknown safety assertion mode '{name}'")),
        }
    }
}

impl SafetyAssertionMode {
    pub fn of(env: &Env, is_method: bool, params: &Parameters) -> Self {
        use crate::library::Type::*;
        if !env.config.generate_safety_asserts {
            return Self::None;
        }
        if is_method {
            return Self::None;
        }
        for par in &params.rust_parameters {
            let c_par = &params.c_parameters[par.ind_c];
            match env.library.type_(c_par.typ) {
                Class(..) | Interface(..)
                    if !*c_par.nullable && c_par.typ.ns_id == library::MAIN_NAMESPACE =>
                {
                    return Self::Skip
                }
                _ => (),
            }
        }

        Self::InMainThread
    }

    pub fn is_none(self) -> bool {
        matches!(self, Self::None)
    }
}
