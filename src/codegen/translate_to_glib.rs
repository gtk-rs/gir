use analysis::conversion_type::ConversionType;
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
            Scalar => format!("{}{}", self.name, ".to_glib()"),
            Pointer => {
                if self.instance_parameter {
                    format!("self{}", to_glib_xxx(self.transfer))
                }
                else {
                    format!("{}{}", self.name, to_glib_xxx(self.transfer))
                }
            }
            Unknown => format!("/*Unknown conversion*/{}", self.name),
        }
    }
}

fn to_glib_xxx(transfer: library::Transfer) -> &'static str {
    use library::Transfer::*;
    match transfer {
        None => ".to_glib_none().0",
        Full => ".to_glib_full()",
        Container => ".to_glib_container()",
    }
}
