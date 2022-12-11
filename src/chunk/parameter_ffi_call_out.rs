use crate::{
    analysis::{self, try_from_glib::TryFromGlib},
    library,
};

#[derive(Clone, Debug)]
pub struct Parameter {
    pub name: String,
    pub typ: library::TypeId,
    pub transfer: library::Transfer,
    pub caller_allocates: bool,
    pub is_error: bool,
    pub is_uninitialized: bool,
    pub try_from_glib: TryFromGlib,
}

impl Parameter {
    pub fn new(orig: &analysis::function_parameters::CParameter, is_uninitialized: bool) -> Self {
        Self {
            name: orig.name.clone(),
            typ: orig.typ,
            transfer: orig.transfer,
            caller_allocates: orig.caller_allocates,
            is_error: orig.is_error,
            is_uninitialized,
            try_from_glib: orig.try_from_glib.clone(),
        }
    }
}

impl From<&analysis::Parameter> for Parameter {
    fn from(orig: &analysis::Parameter) -> Self {
        Self {
            name: orig.lib_par.name.clone(),
            typ: orig.lib_par.typ,
            transfer: orig.lib_par.transfer,
            caller_allocates: orig.lib_par.caller_allocates,
            is_error: orig.lib_par.is_error,
            is_uninitialized: false,
            try_from_glib: orig.try_from_glib.clone(),
        }
    }
}
