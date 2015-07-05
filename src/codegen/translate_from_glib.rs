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
            let rust_type = rust_type(library, self.typ);
            if rust_type.as_str() != func.class_name.as_str() {
                let from_glib_xxx = from_glib_xxx(self.transfer);
                (
                    format!("{}::{}", rust_type.as_str(), from_glib_xxx.0),
                    format!("{}.downcast_unchecked()",from_glib_xxx.1)
                )
            } else {
                ("TODO:constructors_body ".into(), String::new())
            }
        } else {
            match kind {
                TypeKind::Converted => ("from_glib(".into(), ")".into()),
                TypeKind::Direct |
                    TypeKind::Enumeration => (String::new(), String::new()),
                TypeKind::Object => from_glib_xxx(self.transfer),
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
