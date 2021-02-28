use crate::{analysis::function_parameters::Parameters, env::Env, library};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyAssertionMode {
    None,
    Skip,
    NotInitialized,
    InMainThread,
}

impl FromStr for SafetyAssertionMode {
    type Err = String;
    fn from_str(name: &str) -> Result<SafetyAssertionMode, String> {
        use self::SafetyAssertionMode::*;
        match name {
            "none" => Ok(None),
            "skip" => Ok(Skip),
            "not-initialized" => Ok(NotInitialized),
            "in-main-thread" => Ok(InMainThread),
            _ => Err(format!("Unknown safety assertion mode '{}'", name)),
        }
    }
}

impl Default for SafetyAssertionMode {
    fn default() -> SafetyAssertionMode {
        SafetyAssertionMode::None
    }
}

impl SafetyAssertionMode {
    pub fn of(env: &Env, is_method: bool, params: &Parameters) -> SafetyAssertionMode {
        use self::SafetyAssertionMode::*;
        use crate::library::Type::*;
        if !env.config.generate_safety_asserts {
            return None;
        }
        if is_method {
            return None;
        }
        for par in &params.rust_parameters {
            let c_par = &params.c_parameters[par.ind_c];
            match *env.library.type_(c_par.typ) {
                Class(..) | Interface(..)
                    if !*c_par.nullable && c_par.typ.ns_id == library::MAIN_NAMESPACE =>
                {
                    return Skip
                }
                _ => (),
            }
        }

        InMainThread
    }

    pub fn is_none(self) -> bool {
        matches!(self, SafetyAssertionMode::None)
    }
}
