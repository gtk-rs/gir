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
            ToGlibScalar { ref name, .. } => format!("{}{}", name, ".to_glib()"),
            ToGlibPointer {
                ref name,
                instance_parameter,
                transfer,
                ref_mode,
                ref to_glib_extra,
                ref pointer_cast,
                ref explicit_target_type,
                in_trait,
                nullable,
            } => {
                let (left, right) = to_glib_xxx(transfer, ref_mode, explicit_target_type);
                let to_glib_extra = if nullable && !to_glib_extra.is_empty() {
                    format!(".map(|p| p{})", to_glib_extra)
                } else {
                    to_glib_extra.clone()
                };

                if instance_parameter {
                    format!(
                        "{}self{}{}{}",
                        left,
                        if in_trait { to_glib_extra } else { "".into() },
                        right,
                        pointer_cast
                    )
                } else {
                    format!("{}{}{}{}{}", left, name, to_glib_extra, right, pointer_cast)
                }
            }
            ToGlibBorrow => "/*Not applicable conversion Borrow*/".to_owned(),
            ToGlibUnknown { ref name } => format!("/*Unknown conversion*/{}", name),
            ToSome(ref name) => format!("Some({})", name),
            IntoRaw(ref name) => format!("Box_::into_raw({}) as *mut _", name),
            _ => unreachable!("Unexpected transformation type {:?}", self),
        }
    }
}

fn to_glib_xxx(
    transfer: Transfer,
    ref_mode: RefMode,
    explicit_target_type: &str,
) -> (String, &'static str) {
    use self::Transfer::*;
    match transfer {
        None => {
            match ref_mode {
                RefMode::None => ("".into(), ".to_glib_none_mut().0"), //unreachable!(),
                RefMode::ByRef => {
                    if explicit_target_type.is_empty() {
                        ("".into(), ".to_glib_none().0")
                    } else {
                        (
                            format!("ToGlibPtr::<{}>::to_glib_none(", explicit_target_type),
                            ").0",
                        )
                    }
                }
                RefMode::ByRefMut => ("".into(), ".to_glib_none_mut().0"),
                RefMode::ByRefImmut => ("mut_override(".into(), ".to_glib_none().0)"),
                RefMode::ByRefConst => ("const_override(".into(), ".to_glib_none().0)"),
                RefMode::ByRefFake => ("".into(), ""), //unreachable!(),
            }
        }
        Full => ("".into(), ".to_glib_full()"),
        Container => ("".into(), ".to_glib_container().0"),
    }
}
