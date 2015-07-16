use analysis;
use analysis::rust_type::rust_type;
use analysis::type_kind::TypeKind;
use env::Env;
use library;
use traits::*;

pub trait TranslateFromGlib {
    fn translate_from_glib_as_function(&self, env: &Env,
        func: &analysis::functions::Info) -> (String, String);
}

impl TranslateFromGlib for library::Parameter {
    fn translate_from_glib_as_function(&self,
        env: &Env, func: &analysis::functions::Info) -> (String, String) {
        let kind = TypeKind::of(&env.library, self.typ);
        if func.kind == library::FunctionKind::Constructor {
            let rust_type = rust_type(env, self.typ);
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
                //TODO: check gtk_dialog_get_content_area <type name="Box" c:type="GtkWidget*"/>
                TypeKind::Object => from_glib_xxx(self.transfer),
                _ => ("TODO:".into(), String::new()),
            }
        }
    }
}

impl TranslateFromGlib for Option<library::Parameter> {
    fn translate_from_glib_as_function(&self, env: &Env,
        func: &analysis::functions::Info) -> (String, String) {
        match self {
            &Some(ref par) => par.translate_from_glib_as_function(env, func),
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
