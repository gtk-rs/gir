use analysis::conversion_type::ConversionType;
use analysis::ref_mode::RefMode;
use chunk;
use library;

pub trait TranslateToGlib {
    fn translate_to_glib(&self, library: &library::Library) -> String;
}

impl TranslateToGlib for chunk::parameter_ffi_call_in::Parameter {
    fn translate_to_glib(&self, library: &library::Library) -> String {
        use analysis::conversion_type::ConversionType::*;
        match ConversionType::of(library, self.typ) {
            Direct => self.name.clone(),
            Scalar => {
                format!(
                    "{}{}{}",
                    self.name,
                    if !*self.nullable { "" } else { ".into()" },
                    ".to_glib()"
                )
            }
            Pointer => {
                let (left, right) = to_glib_xxx(self.transfer, self.ref_mode, self.is_into);
                if self.instance_parameter {
                    format!("{}self{}", left, right)
                } else {
                    format!("{}{}{}{}", left, self.name, self.to_glib_extra, right)
                }
            }
            Borrow => "/*Not applicable conversion Borrow*/".to_owned(),
            Unknown => format!("/*Unknown conversion*/{}", self.name),
        }
    }
}

fn to_glib_xxx(
    transfer: library::Transfer,
    ref_mode: RefMode,
    is_into: bool,
) -> (&'static str, &'static str) {
    use library::Transfer::*;
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
