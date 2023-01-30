use crate::{
    analysis::{
        self, conversion_type::ConversionType, rust_type::RustType, try_from_glib::TryFromGlib,
    },
    chunk::conversion_from_glib::Mode,
    env::Env,
    library,
    nameutil::use_glib_type,
    traits::*,
};

pub trait TranslateFromGlib {
    fn translate_from_glib_as_function(
        &self,
        env: &Env,
        array_length: Option<&str>,
    ) -> (String, String);
}

impl TranslateFromGlib for Mode {
    fn translate_from_glib_as_function(
        &self,
        env: &Env,
        array_length: Option<&str>,
    ) -> (String, String) {
        use crate::analysis::conversion_type::ConversionType::*;
        match ConversionType::of(env, self.typ) {
            Direct => (String::new(), String::new()),
            Scalar => match env.library.type_(self.typ) {
                library::Type::Basic(library::Basic::UniChar) => (
                    "std::convert::TryFrom::try_from(".into(),
                    ").expect(\"conversion from an invalid Unicode value attempted\")".into(),
                ),
                _ => ("from_glib(".into(), ")".into()),
            },
            Option => {
                let (pre, post) = match &self.try_from_glib {
                    TryFromGlib::Option => ("from_glib(", ")"),
                    TryFromGlib::OptionMandatory => (
                        "try_from_glib(",
                        ").expect(\"mandatory glib value is None\")",
                    ),
                    other => panic!("Unexpected {other:?} for ConversionType::Option"),
                };
                (pre.to_string(), post.to_string())
            }
            Result { .. } => {
                let (pre, post) = match &self.try_from_glib {
                    TryFromGlib::Result { .. } => ("try_from_glib(", ")"),
                    TryFromGlib::ResultInfallible { .. } => (
                        "try_from_glib(",
                        ").unwrap_or_else(|err| panic!(\"infallible {}\", err))",
                    ),
                    other => panic!("Unexpected {other:?} for ConversionType::Result"),
                };
                (pre.to_string(), post.to_string())
            }
            Pointer => {
                let trans = from_glib_xxx(self.transfer, array_length);
                match env.type_(self.typ) {
                    library::Type::List(..)
                    | library::Type::SList(..)
                    | library::Type::PtrArray(..)
                    | library::Type::CArray(..) => {
                        if array_length.is_some() {
                            (format!("FromGlibContainer::{}", trans.0), trans.1)
                        } else {
                            (format!("FromGlibPtrContainer::{}", trans.0), trans.1)
                        }
                    }
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
        array_length: Option<&str>,
    ) -> (String, String) {
        match self.parameter {
            Some(ref par) => match self.base_tid {
                Some(tid) => {
                    let rust_type = RustType::builder(env, tid)
                        .direction(par.lib_par.direction)
                        .try_from_glib(&par.try_from_glib)
                        .try_build();
                    let from_glib_xxx = from_glib_xxx(par.lib_par.transfer, None);

                    let prefix = if *par.lib_par.nullable {
                        format!("Option::<{}>::{}", rust_type.into_string(), from_glib_xxx.0)
                    } else {
                        format!("{}::{}", rust_type.into_string(), from_glib_xxx.0)
                    };
                    let suffix_function = if *par.lib_par.nullable {
                        "map(|o| o.unsafe_cast())"
                    } else {
                        "unsafe_cast()"
                    };

                    if let Some(ref msg) = self.nullable_return_is_error {
                        assert!(*par.lib_par.nullable);
                        (
                            prefix,
                            format!(
                                "{}.{}.ok_or_else(|| {}(\"{}\"))",
                                from_glib_xxx.1,
                                suffix_function,
                                use_glib_type(env, "bool_error!"),
                                msg
                            ),
                        )
                    } else {
                        (prefix, format!("{}.{}", from_glib_xxx.1, suffix_function))
                    }
                }
                None if self.bool_return_is_error.is_some() => (
                    use_glib_type(env, "result_from_gboolean!("),
                    format!(", \"{}\")", self.bool_return_is_error.as_ref().unwrap()),
                ),
                None if self.nullable_return_is_error.is_some() => {
                    let res = Mode::from(par).translate_from_glib_as_function(env, array_length);
                    if let Some(ref msg) = self.nullable_return_is_error {
                        assert!(*par.lib_par.nullable);
                        (
                            format!("Option::<_>::{}", res.0),
                            format!(
                                "{}.ok_or_else(|| {}(\"{}\"))",
                                res.1,
                                use_glib_type(env, "bool_error!"),
                                msg
                            ),
                        )
                    } else {
                        res
                    }
                }
                None => Mode::from(par).translate_from_glib_as_function(env, array_length),
            },
            None => (String::new(), ";".into()),
        }
    }
}

fn from_glib_xxx(transfer: library::Transfer, array_length: Option<&str>) -> (String, String) {
    use crate::library::Transfer;
    let good_print = |name: &str| format!(", {name}.assume_init() as _)");
    match (transfer, array_length) {
        (Transfer::None, None) => ("from_glib_none(".into(), ")".into()),
        (Transfer::Full, None) => ("from_glib_full(".into(), ")".into()),
        (Transfer::Container, None) => ("from_glib_container(".into(), ")".into()),
        (Transfer::None, Some(array_length_name)) => {
            ("from_glib_none_num(".into(), good_print(array_length_name))
        }
        (Transfer::Full, Some(array_length_name)) => {
            ("from_glib_full_num(".into(), good_print(array_length_name))
        }
        (Transfer::Container, Some(array_length_name)) => (
            "from_glib_container_num(".into(),
            good_print(array_length_name),
        ),
    }
}
