use library;
use super::parameter_ffi_call_out;

#[derive(Clone)]
pub struct Mode {
    pub typ: library::TypeId,
    pub transfer: library::Transfer,
}

impl<'a> From<&'a parameter_ffi_call_out::Parameter> for Mode {
    fn from(orig: &'a parameter_ffi_call_out::Parameter) -> Mode {
        Mode {
            typ: orig.typ,
            transfer: orig.transfer,
        }
    }
}

impl<'a> From<&'a library::Parameter> for Mode {
    fn from(orig: &'a library::Parameter) -> Mode {
        Mode {
            typ: orig.typ,
            transfer: orig.transfer,
        }
    }
}
