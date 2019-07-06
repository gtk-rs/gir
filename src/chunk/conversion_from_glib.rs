use super::parameter_ffi_call_out;
use crate::library;

#[derive(Clone, Debug)]
pub struct Mode {
    pub typ: library::TypeId,
    pub transfer: library::Transfer,
    pub is_uninitialized: bool,
}

impl<'a> From<&'a parameter_ffi_call_out::Parameter> for Mode {
    fn from(orig: &'a parameter_ffi_call_out::Parameter) -> Mode {
        Mode {
            typ: orig.typ,
            transfer: orig.transfer,
            is_uninitialized: orig.is_uninitialized,
        }
    }
}

impl<'a> From<&'a library::Parameter> for Mode {
    fn from(orig: &'a library::Parameter) -> Mode {
        Mode {
            typ: orig.typ,
            transfer: orig.transfer,
            is_uninitialized: false,
        }
    }
}
