use super::parameter_ffi_call_out;
use crate::{
    analysis::{self, try_from_glib::TryFromGlib},
    library::{self, Nullable},
};

#[derive(Clone, Debug)]
pub struct Mode {
    pub typ: library::TypeId,
    pub transfer: library::Transfer,
    pub try_from_glib: TryFromGlib,
    pub nullable: Nullable,
}

impl From<&parameter_ffi_call_out::Parameter> for Mode {
    fn from(orig: &parameter_ffi_call_out::Parameter) -> Mode {
        Mode {
            typ: orig.typ,
            transfer: orig.transfer,
            try_from_glib: orig.try_from_glib.clone(),
            nullable: orig.nullable,
        }
    }
}

impl From<&analysis::Parameter> for Mode {
    fn from(orig: &analysis::Parameter) -> Mode {
        Mode {
            typ: orig.lib_par.typ,
            transfer: orig.lib_par.transfer,
            try_from_glib: orig.try_from_glib.clone(),
            nullable: orig.nullable,
        }
    }
}
