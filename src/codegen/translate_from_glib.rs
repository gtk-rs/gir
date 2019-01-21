use analysis;
use analysis::rust_type::rust_type;
use analysis::conversion_type::ConversionType;
use chunk::conversion_from_glib::Mode;
use env::Env;
use library;
use traits::*;

pub trait TranslateFromGlib {
    fn translate_from_glib_as_function(
        &self,
        env: &Env,
        array_length: Option<&String>,
    ) -> (String, String);
}

impl TranslateFromGlib for Mode {
    fn translate_from_glib_as_function(
        &self,
        env: &Env,
        array_length: Option<&String>,
    ) -> (String, String) {
        use analysis::conversion_type::ConversionType::*;
        match ConversionType::of(env, self.typ) {
            Direct => (String::new(), String::new()),
            Scalar => ("from_glib(".into(), ")".into()),
            Pointer => {
                let trans = from_glib_xxx(self.transfer, array_length);
                match *env.type_(self.typ) {
                    library::Type::List(..) |
                    library::Type::SList(..) |
                    library::Type::CArray(..) => if array_length.is_some() {
                        (format!("FromGlibContainer::{}", trans.0), trans.1)
                    } else {
                        (format!("FromGlibPtrContainer::{}", trans.0), trans.1)
                    },
                    _ => trans,
                }
            }
            Borrow => ("/*TODO: conversion Borrow*/".into(), String::new()),
            Unknown => ("/*Unknown conversion*/".into(), String::new()),
        }
    }
}

impl TranslateFromGlib for analysis::return_value::Info {
    fn translate_from_glib_as_function(
        &self,
        env: &Env,
        array_length: Option<&String>,
    ) -> (String, String) {
        match self.parameter {
            Some(ref par) => match self.base_tid {
                Some(tid) => {
                    let rust_type = rust_type(env, tid);
                    let from_glib_xxx = from_glib_xxx(par.transfer, None);

                    let prefix = if *par.nullable {
                        format!("Option::<{}>::{}", rust_type.into_string(), from_glib_xxx.0)
                    } else {
                        format!("{}::{}", rust_type.into_string(), from_glib_xxx.0)
                    };
                    let suffix_function = if *par.nullable {
                        "map(|o| o.unsafe_cast())"
                    } else {
                        "unsafe_cast()"
                    };
                    (prefix, format!("{}.{}", from_glib_xxx.1, suffix_function))
                }
                None if self.bool_return_is_error.is_some() => {
                    (
                        "glib_result_from_gboolean!(".into(),
                        format!(", \"{}\")", self.bool_return_is_error.as_ref().unwrap()),
                    )
                }
                None => Mode::from(par).translate_from_glib_as_function(env, array_length),
            },
            None => (String::new(), ";".into()),
        }
    }
}

fn from_glib_xxx(transfer: library::Transfer, array_length: Option<&String>) -> (String, String) {
    use library::Transfer;
    match (transfer, array_length) {
        (Transfer::None, None) => ("from_glib_none(".into(), ")".into()),
        (Transfer::Full, None) => ("from_glib_full(".into(), ")".into()),
        (Transfer::Container, None) => ("from_glib_container(".into(), ")".into()),
        (Transfer::None, Some(array_length_name)) => (
            "from_glib_none_num(".into(),
            format!(", {} as usize)", array_length_name),
        ),
        (Transfer::Full, Some(array_length_name)) => (
            "from_glib_full_num(".into(),
            format!(", {} as usize)", array_length_name),
        ),
        (Transfer::Container, Some(array_length_name)) => (
            "from_glib_container_num(".into(),
            format!(", {} as usize)", array_length_name),
        ),
    }
}
