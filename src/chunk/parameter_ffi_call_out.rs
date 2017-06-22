use analysis;
use library;

#[derive(Clone)]
pub struct Parameter {
    pub name: String,
    pub typ: library::TypeId,
    pub transfer: library::Transfer,
    pub caller_allocates: bool,
    pub array_length: Option<(String, String)>,
    pub is_error: bool,
}

impl Parameter {
    pub fn new(
        orig: &analysis::parameter::Parameter,
        array_length: Option<(String, String)>,
    ) -> Parameter {
        Parameter {
            name: orig.name.clone(),
            typ: orig.typ,
            transfer: orig.transfer,
            array_length: array_length,
            caller_allocates: orig.caller_allocates,
            is_error: orig.is_error,
        }
    }
}
