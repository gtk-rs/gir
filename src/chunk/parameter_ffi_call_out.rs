use analysis;
use library;

#[derive(Clone, Debug)]
pub struct Parameter {
    pub name: String,
    pub typ: library::TypeId,
    pub transfer: library::Transfer,
    pub caller_allocates: bool,
    pub is_error: bool,
    pub nullable: library::Nullable
}

impl Parameter {
    pub fn new(orig: &analysis::function_parameters::CParameter) -> Parameter {
        Parameter {
            name: orig.name.clone(),
            typ: orig.typ,
            transfer: orig.transfer,
            caller_allocates: orig.caller_allocates,
            is_error: orig.is_error,
            nullable: orig.nullable,
        }
    }
}

impl<'a> From<&'a library::Parameter> for Parameter {
    fn from(orig: &'a library::Parameter) -> Parameter{
        Parameter {
            name: orig.name.clone(),
            typ: orig.typ,
            transfer: orig.transfer,
            caller_allocates: orig.caller_allocates,
            is_error: orig.is_error,
            nullable: orig.nullable
        }
    }
}
