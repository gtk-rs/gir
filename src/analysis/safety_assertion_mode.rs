use analysis::function_parameters::Parameters;
use env::Env;
use library;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyAssertionMode {
    None,
    Skip,
    InMainThread,
}

impl Default for SafetyAssertionMode {
    fn default() -> SafetyAssertionMode {
        SafetyAssertionMode::None
    }
}

impl SafetyAssertionMode {
    pub fn of(env: &Env, is_method: bool, params: &Parameters) -> SafetyAssertionMode {
        use self::SafetyAssertionMode::*;
        use library::Type::*;
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
                    if !*c_par.nullable && c_par.typ.ns_id == library::MAIN_NAMESPACE => {
                    return Skip
                }
                _ => (),
            }
        }

        InMainThread
    }
}
