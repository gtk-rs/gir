use analysis;

#[derive(Clone)]
pub struct Parameter {
    pub name: String,
    pub ref_mode: analysis::ref_mode::RefMode,
    pub is_into: bool,
}

impl Parameter {
    pub fn new(orig: &analysis::function_parameters::CParameter) -> Parameter {
        Parameter {
            name: orig.name.clone(),
            ref_mode: orig.ref_mode,
            is_into: orig.is_into,
        }
    }
}
