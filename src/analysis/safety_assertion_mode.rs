use analysis::parameter::Parameter;
use env::Env;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyAssertionMode {
    None,
    InMainThread,
}

impl Default for SafetyAssertionMode {
    fn default() -> SafetyAssertionMode {
        SafetyAssertionMode::None
    }
}

impl SafetyAssertionMode {
    pub fn of(env: &Env, params: &[Parameter]) -> SafetyAssertionMode {
        use self::SafetyAssertionMode::*;
        if !env.config.generate_safety_asserts {
            return None;
        }
        if params.len() > 0 && params[0].instance_parameter {
            return None;
        }

        InMainThread
    }
}
