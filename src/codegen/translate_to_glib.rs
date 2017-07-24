use analysis::function_parameters::TransformationType;
use analysis::ref_mode::RefMode;
use library::Transfer;

pub trait TranslateToGlib {
    fn translate_to_glib(&self) -> String;
}

impl TranslateToGlib for TransformationType {
    fn translate_to_glib(&self) -> String {
        use self::TransformationType::*;
        match *self {
            ToGlibDirect { ref name } => name.clone(),
            ToGlibScalar { ref name, nullable } => {
                format!(
                    "{}{}{}",
                    name,
                    if !*nullable { "" } else { ".into()" },
                    ".to_glib()"
                )
            }
            ToGlibPointer {
                ref name,
                instance_parameter,
                transfer,
                ref_mode,
                ref to_glib_extra,
                is_into,
            } => {
                let (left, right) = to_glib_xxx(transfer, ref_mode, is_into);
                if instance_parameter {
                    format!("{}self{}", left, right)
                } else {
                    format!("{}{}{}{}", left, name, to_glib_extra, right)
                }
            }
            ToGlibBorrow => "/*Not applicable conversion Borrow*/".to_owned(),
            ToGlibUnknown { ref name } => format!("/*Unknown conversion*/{}", name),
            _ => unreachable!("Unexpected transformation type {:?}", self),
        }
    }
}

fn to_glib_xxx(
    transfer: Transfer,
    ref_mode: RefMode,
    is_into: bool,
) -> (&'static str, &'static str) {
    use self::Transfer::*;
    match transfer {
        None => {
            match ref_mode {
                RefMode::None => ("", ".to_glib_none_mut().0"),//unreachable!(),
                RefMode::ByRef if is_into => ("", ".0"),
                RefMode::ByRef => ("", ".to_glib_none().0"),
                RefMode::ByRefMut => ("", ".to_glib_none_mut().0"),
                RefMode::ByRefImmut => ("mut_override(", ".to_glib_none().0)"),
                RefMode::ByRefFake => ("", ""),//unreachable!(),
            }
        }
        Full => ("", ".to_glib_full()"),
        Container => ("", ".to_glib_container().0"),
    }
}
