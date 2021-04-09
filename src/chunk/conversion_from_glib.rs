use super::parameter_ffi_call_out;
use crate::{
    analysis::{self, try_from_glib::TryFromGlib},
    library,
};

#[derive(Clone, Debug)]
pub struct Mode {
    pub typ: library::TypeId,
    pub transfer: library::Transfer,
    pub is_uninitialized: bool,
    pub try_from_glib: TryFromGlib,
}

impl From<&parameter_ffi_call_out::Parameter> for Mode {
    fn from(orig: &parameter_ffi_call_out::Parameter) -> Mode {
        Mode {
            typ: orig.typ,
            transfer: orig.transfer,
            is_uninitialized: orig.is_uninitialized,
            try_from_glib: orig.try_from_glib.clone(),
        }
    }
}

impl From<&analysis::Parameter> for Mode {
    fn from(orig: &analysis::Parameter) -> Mode {
        Mode {
            typ: orig.lib_par.typ,
            transfer: orig.lib_par.transfer,
            is_uninitialized: false,
            try_from_glib: orig.try_from_glib.clone(),
        }
    }
}
