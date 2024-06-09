use crate::{analysis::conversion_type::ConversionType, env, library};

pub trait TrampolineToGlib {
    fn trampoline_to_glib(&self, env: &env::Env) -> String;
}

impl TrampolineToGlib for library::Parameter {
    fn trampoline_to_glib(&self, env: &env::Env) -> String {
        use crate::analysis::conversion_type::ConversionType::*;
        match ConversionType::of(env, self.typ()) {
            Direct => String::new(),
            Scalar | Option | Result { .. } => ".into_glib()".to_owned(),
            Pointer => to_glib_xxx(self.transfer_ownership()).to_owned(),
            Borrow => "/*Not applicable conversion Borrow*/".to_owned(),
            Unknown => "/*Unknown conversion*/".to_owned(),
        }
    }
}

fn to_glib_xxx(transfer: gir_parser::TransferOwnership) -> &'static str {
    match transfer {
        gir_parser::TransferOwnership::None => "/*Not checked*/.to_glib_none().0",
        gir_parser::TransferOwnership::Full => ".to_glib_full()",
        gir_parser::TransferOwnership::Container => "/*Not checked*/.to_glib_container().0",
    }
}
