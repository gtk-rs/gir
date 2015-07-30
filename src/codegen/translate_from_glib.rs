use analysis;
use analysis::rust_type::rust_type;
use analysis::type_kind::TypeKind;
use env::Env;
use library;
use traits::*;

pub trait TranslateFromGlib {
    fn translate_from_glib_as_function(&self, env: &Env) -> (String, String);
}

//TODO: move to code for analysis::return_value::Info
impl TranslateFromGlib for library::Parameter {
    fn translate_from_glib_as_function(&self, env: &Env) -> (String, String) {
        let kind = TypeKind::of(&env.library, self.typ);
        match kind {
            TypeKind::Converted => ("from_glib(".into(), ")".into()),
            TypeKind::Direct |
                TypeKind::Enumeration => (String::new(), String::new()),
            //TODO: check gtk_dialog_get_content_area <type name="Box" c:type="GtkWidget*"/>
            TypeKind::Pointer | //Checked only for Option<String>
                TypeKind::Object => from_glib_xxx(self.transfer),
            _ => (format!("TODO {:?}:", kind), String::new()),
        }
    }
}

impl TranslateFromGlib for analysis::return_value::Info {
    fn translate_from_glib_as_function(&self, env: &Env) -> (String, String) {
        match self.parameter {
            Some(ref par) => match self.base_tid {
                Some(tid) => {
                    let rust_type = rust_type(env, tid);
                    let from_glib_xxx = from_glib_xxx(par.transfer);
                    (
                        format!("{}::{}", rust_type.as_str(), from_glib_xxx.0),
                        format!("{}.downcast_unchecked()",from_glib_xxx.1)
                    )
                }
                None => par.translate_from_glib_as_function(env)
            },
            None => (String::new(), ";".into())
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
