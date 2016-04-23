use analysis;
use analysis::rust_type::rust_type;
use analysis::conversion_type::ConversionType;
use chunk::conversion_from_glib::Mode;
use env::Env;
use library;
use traits::*;

pub trait TranslateFromGlib {
    fn translate_from_glib_as_function(&self, env: &Env) -> (String, String);
}

impl TranslateFromGlib for Mode {
    fn translate_from_glib_as_function(&self, env: &Env) -> (String, String) {
        use analysis::conversion_type::ConversionType::*;
        match ConversionType::of(&env.library, self.typ) {
            Direct => (String::new(), String::new()),
            Scalar => ("from_glib(".into(), ")".into()),
            Pointer => {
                let trans = from_glib_xxx(self.transfer);
                match *env.type_(self.typ) {
                    library::Type::List(..) |
                        library::Type::SList(..) |
                        library::Type::CArray(..) => {
                        (format!("FromGlibPtrContainer::{}", trans.0), trans.1)
                    }
                    _ => trans,
                }
            }
            Unknown => ("/*Unknown conversion*/".into(), String::new()),
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
                    let prefix = if *par.nullable {
                        format!("Option::<{}>::{}", rust_type.to_cow_str(), from_glib_xxx.0)
                    } else {
                        format!("{}::{}", rust_type.to_cow_str(), from_glib_xxx.0)
                    };
                    let suffix_function = if *par.nullable {
                        "map(Downcast::downcast_unchecked)"
                    } else {
                        "downcast_unchecked()"
                    };
                    (
                        prefix,
                        format!("{}.{}", from_glib_xxx.1, suffix_function)
                    )
                }
                None => Mode::from(par).translate_from_glib_as_function(env),
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
