use analysis::parameter::Parameter;
use analysis::rust_type::rust_type;
use analysis::conversion_type::ConversionType;
use env::Env;
use library;
use traits::*;

pub trait TrampolineFromGlib {
    fn trampoline_from_glib(&self, env: &Env, need_downcast: bool) -> String;
}

impl TrampolineFromGlib for Parameter {
    fn trampoline_from_glib(&self, env: &Env, need_downcast: bool) -> String {
        use analysis::conversion_type::ConversionType::*;
        match ConversionType::of(&env.library, self.typ) {
            Direct => self.name.clone(),
            Scalar => format!("from_glib({})", self.name),
            Pointer => {
                let (mut left, mut right) = from_glib_xxx(self.transfer);
                let type_name = rust_type(env, self.typ).into_string();
                left = format!("&{}::{}", type_name, left);
                if need_downcast {
                    right = format!("{}.downcast_unchecked()", right);
                }
                format!("{}{}{}", left, self.name, right)
            }
            Unknown => format!("/*Unknown conversion*/{}", self.name)
        }
    }
}

fn from_glib_xxx(transfer: library::Transfer) -> (String, String) {
    use library::Transfer::*;
    match transfer {
        None => ("from_glib_none(".into(), ")".into()),
        Full => ("from_glib_full(".into(), ")".into()),
        Container => ("from_glib_container(".into(), ")".into()),
    }
}
