use crate::{
    analysis::{rust_type::RustType, trampoline_parameters::Transformation},
    env::Env,
    library,
    nameutil::is_gstring,
    traits::*,
};

pub trait TrampolineFromGlib {
    fn trampoline_from_glib(&self, env: &Env, need_downcast: bool, nullable: bool) -> String;
}

impl TrampolineFromGlib for Transformation {
    fn trampoline_from_glib(&self, env: &Env, need_downcast: bool, nullable: bool) -> String {
        use crate::analysis::conversion_type::ConversionType::*;
        let need_type_name = need_downcast || is_need_type_name(env, self.typ);
        match self.conversion_type {
            Direct => self.name.clone(),
            Scalar | Option | Result { .. } => format!("from_glib({})", self.name),
            Borrow | Pointer => {
                let is_borrow = self.conversion_type == Borrow;
                let need_type_name = need_type_name || (is_borrow && nullable);
                let (mut left, mut right) = from_glib_xxx(self.transfer, is_borrow);
                let type_name = RustType::try_new(env, self.typ).into_string();
                if need_type_name {
                    if is_borrow && nullable {
                        left = format!("Option::<{type_name}>::{left}");
                    } else {
                        left = format!("{type_name}::{left}");
                    }
                }

                if !nullable {
                    left = format!(
                        "{}{}",
                        if need_downcast && is_borrow { "" } else { "&" },
                        left
                    );
                } else if nullable && is_borrow {
                    if is_gstring(&type_name) {
                        right = format!("{right}.as_ref().as_ref().map(|s| s.as_str())");
                    } else {
                        right = format!("{right}.as_ref().as_ref()");
                    }
                } else if is_gstring(&type_name) {
                    right = format!("{right}.as_ref().map(|s| s.as_str())");
                } else {
                    right = format!("{right}.as_ref()");
                }

                if need_downcast && is_borrow {
                    right = format!("{right}.unsafe_cast_ref()");
                } else if need_downcast {
                    right = format!("{right}.unsafe_cast()");
                }

                format!("{}{}{}", left, self.name, right)
            }
            Unknown => format!("/*Unknown conversion*/{}", self.name),
        }
    }
}

pub fn from_glib_xxx(transfer: library::Transfer, is_borrow: bool) -> (String, String) {
    use crate::library::Transfer::*;
    match transfer {
        None if is_borrow => ("from_glib_borrow(".into(), ")".into()),
        None => ("from_glib_none(".into(), ")".into()),
        Full => ("from_glib_full(".into(), ")".into()),
        Container => ("from_glib_container(".into(), ")".into()),
    }
}

fn is_need_type_name(env: &Env, type_id: library::TypeId) -> bool {
    if type_id.ns_id == library::INTERNAL_NAMESPACE {
        use crate::library::{Basic::*, Type::*};
        matches!(env.type_(type_id), Basic(Utf8 | Filename | OsString))
    } else {
        false
    }
}
