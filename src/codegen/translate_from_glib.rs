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
            TypeKind::Pointer | //Checked only for Option<String>
                TypeKind::Object => from_glib_xxx(self.transfer),
            TypeKind::Container => {
                let trans = from_glib_xxx(self.transfer);
                (format!("FromGlibPtrContainer::{}", trans.0), trans.1)
            }
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
                    let prefix = if par.nullable {
                        format!("Option::<{}>::{}", rust_type.as_str(), from_glib_xxx.0)
                    } else {
                        format!("{}::{}", rust_type.as_str(), from_glib_xxx.0)
                    };
                    let suffix_function = if par.nullable {
                        "map(Downcast::downcast_unchecked)"
                    } else {
                        "downcast_unchecked()"
                    };
                    (
                        prefix,
                        format!("{}.{}", from_glib_xxx.1, suffix_function)
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
        Container => ("from_glib_container(".into(), ")".into()),
    }
}
