use super::parameter_ffi_call_out;
use crate::{
    analysis::{self, try_from_glib::TryFromGlib},
    library,
};

#[derive(Clone, Debug)]
pub struct Mode {
    pub typ: library::TypeId,
    pub transfer: gir_parser::TransferOwnership,
    pub try_from_glib: TryFromGlib,
}

impl From<&parameter_ffi_call_out::Parameter> for Mode {
    fn from(orig: &parameter_ffi_call_out::Parameter) -> Mode {
        Mode {
            typ: orig.typ,
            transfer: orig.transfer,
            try_from_glib: orig.try_from_glib.clone(),
        }
    }
}

impl From<&analysis::Parameter> for Mode {
    fn from(orig: &analysis::Parameter) -> Mode {
        Mode {
            typ: orig.lib_par.typ(),
            transfer: orig.lib_par.transfer_ownership(),
            try_from_glib: orig.try_from_glib.clone(),
        }
    }
}
