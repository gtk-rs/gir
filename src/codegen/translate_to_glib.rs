use crate::{
    analysis::{function_parameters::TransformationType, ref_mode::RefMode},
    library::Transfer,
};

pub trait TranslateToGlib {
    fn translate_to_glib(&self) -> String;
}

impl TranslateToGlib for TransformationType {
    fn translate_to_glib(&self) -> String {
        use self::TransformationType::*;
        match *self {
            ToGlibDirect { ref name } => name.clone(),
            ToGlibScalar {
                ref name,
                needs_into,
                ..
            } => {
                let pre_into = if needs_into { ".into()" } else { "" };
                format!("{}{}{}", name, pre_into, ".into_glib()")
            }
            ToGlibPointer {
                ref name,
                instance_parameter,
                transfer,
                ref_mode,
                ref to_glib_extra,
                ref pointer_cast,
                ref explicit_target_type,
                in_trait,
                move_,
                ..
            } => {
                let (left, right) = to_glib_xxx(transfer, ref_mode, explicit_target_type, move_);

                if instance_parameter {
                    format!(
                        "{}self{}{}{}",
                        left,
                        if in_trait { to_glib_extra } else { "" },
                        right,
                        pointer_cast
                    )
                } else {
                    format!("{left}{name}{to_glib_extra}{right}{pointer_cast}")
                }
            }
            ToGlibBorrow => "/*Not applicable conversion Borrow*/".to_owned(),
            ToGlibUnknown { ref name } => format!("/*Unknown conversion*/{name}"),
            ToSome(ref name) => format!("Some({name})"),
            IntoRaw(ref name) => format!("Box_::into_raw({name}) as *mut _"),
            _ => unreachable!("Unexpected transformation type {:?}", self),
        }
    }
}

fn to_glib_xxx(
    transfer: Transfer,
    ref_mode: RefMode,
    explicit_target_type: &str,
    move_: bool,
) -> (String, &'static str) {
    use self::Transfer::*;
    match transfer {
        None => {
            match ref_mode {
                RefMode::None => (String::new(), ".to_glib_none_mut().0"), // unreachable!(),
                RefMode::ByRef => match (move_, explicit_target_type.is_empty()) {
                    (true, true) => (String::new(), ".into_glib_ptr()"),
                    (true, false) => (
                        format!("ToGlibPtr::<{explicit_target_type}>::into_glib_ptr("),
                        ")",
                    ),
                    (false, true) => (String::new(), ".to_glib_none().0"),
                    (false, false) => (
                        format!("ToGlibPtr::<{explicit_target_type}>::to_glib_none("),
                        ").0",
                    ),
                },
                RefMode::ByRefMut => (String::new(), ".to_glib_none_mut().0"),
                RefMode::ByRefImmut => ("mut_override(".into(), ".to_glib_none().0)"),
                RefMode::ByRefConst => ("const_override(".into(), ".to_glib_none().0)"),
                RefMode::ByRefFake => (String::new(), ""), // unreachable!(),
            }
        }
        Full => {
            if move_ {
                ("".into(), ".into_glib_ptr()")
            } else {
                ("".into(), ".to_glib_full()")
            }
        }
        Container => ("".into(), ".to_glib_container().0"),
    }
}
