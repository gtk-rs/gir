use analysis::parameter::Parameter;

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
    pub fn of(params: &[Parameter]) -> SafetyAssertionMode {
        use self::SafetyAssertionMode::*;
        if params.len() > 0 && params[0].instance_parameter {
            return None;
        }

        InMainThread
    }
}
