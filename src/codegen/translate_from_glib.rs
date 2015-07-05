use analysis;
use analysis::rust_type::{AsStr, rust_type};
use analysis::type_kind::TypeKind;
use library;

pub trait TranslateFromGlib {
    fn translate_from_glib_as_function(&self,
        library: &library::Library, func: &analysis::functions::Info) -> (String, String);
}

impl TranslateFromGlib for library::Parameter {
    fn translate_from_glib_as_function(&self,
        library: &library::Library, func: &analysis::functions::Info) -> (String, String) {
        let kind = TypeKind::of(library, self.typ);
        if func.kind == library::FunctionKind::Constructor {
            match kind {
                TypeKind::SpecialType => (
                    format!("{}::from_glib_none(",
                        rust_type(library, library::SPECIAL_TYPE_ID).as_str()),
                    ").downcast_unchecked()".into()
                ),
                _ => ("TODO:constructors_body ".into(), String::new())
            }
        } else {
            match kind {
                TypeKind::Converted => ("from_glib(".into(), ")".into()),
                TypeKind::Direct |
                    TypeKind::Enumeration => (String::new(), String::new()),
                TypeKind::Object |
                    TypeKind::SpecialType => from_glib_xxx(self.transfer),
                _ => ("TODO:".into(), String::new()),
            }
        }
    }
}

impl TranslateFromGlib for Option<library::Parameter> {
    fn translate_from_glib_as_function(&self,
        library: &library::Library, func: &analysis::functions::Info) -> (String, String) {
        match self {
            &Some(ref par) => par.translate_from_glib_as_function(library, func),
            &None => (String::new(), ";".into())
        }
    }
}

fn from_glib_xxx(transfer: library::Transfer) -> (String, String) {
    use library::Transfer::*;
    match transfer {
        None => ("from_glib_none(".into(), ")".into()),
        Full => ("from_glib_full(".into(), ")".into()),
        Container => ("TODO:".into(), String::new()),
    }
}
