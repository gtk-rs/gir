use analysis::conversion_type::ConversionType;
use library;

pub trait TrampolineToGlib {
    fn trampoline_to_glib(&self, library: &library::Library) -> String;
}

impl TrampolineToGlib for library::Parameter {
    fn trampoline_to_glib(&self, library: &library::Library) -> String {
        use analysis::conversion_type::ConversionType::*;
        match ConversionType::of(library, self.typ) {
            Direct => String::new(),
            Scalar => ".to_glib()".to_owned(),
            Pointer => to_glib_xxx(self.transfer).to_owned(),
            Borrow => "/*Not applicable conversion Borrow*/".to_owned(),
            Unknown => "/*Unknown conversion*/".to_owned(),
        }
    }
}

fn to_glib_xxx(transfer: library::Transfer) -> &'static str {
    use library::Transfer::*;
    match transfer {
        None => "/*Not checked*/.to_glib_none().0",
        Full => ".to_glib_full()",
        Container => "/*Not checked*/.to_glib_container().0",
    }
}
