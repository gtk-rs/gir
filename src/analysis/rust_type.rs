use super::conversion_type::ConversionType;
use crate::{
    analysis::namespaces,
    analysis::ref_mode::RefMode,
    env::Env,
    library::{self, Nullable, ParameterScope},
    traits::*,
};
use std::result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeError {
    Ignored(String),
    Mismatch(String),
    Unimplemented(String),
}

pub type Result = result::Result<String, TypeError>;

fn into_inner(res: Result) -> String {
    use self::TypeError::*;
    match res {
        Ok(s) | Err(Ignored(s)) | Err(Mismatch(s)) | Err(Unimplemented(s)) => s,
    }
}

impl IntoString for Result {
    fn into_string(self) -> String {
        use self::TypeError::*;
        match self {
            Ok(s) => s,
            Err(Ignored(s)) => format!("/*Ignored*/{}", s),
            Err(Mismatch(s)) => format!("/*Metadata mismatch*/{}", s),
            Err(Unimplemented(s)) => format!("/*Unimplemented*/{}", s),
        }
    }
}

impl MapAny<String> for Result {
    fn map_any<F: FnOnce(String) -> String>(self, op: F) -> Result {
        use self::TypeError::*;
        match self {
            Ok(s) => Ok(op(s)),
            Err(Ignored(s)) => Err(Ignored(op(s))),
            Err(Mismatch(s)) => Err(Mismatch(op(s))),
            Err(Unimplemented(s)) => Err(Unimplemented(op(s))),
        }
    }
}

pub fn rust_type(env: &Env, type_id: library::TypeId) -> Result {
    rust_type_full(
        env,
        type_id,
        Nullable(false),
        RefMode::None,
        ParameterScope::None,
        library::Concurrency::None,
    )
}

pub fn rust_type_nullable(env: &Env, type_id: library::TypeId, nullable: Nullable) -> Result {
    rust_type_full(
        env,
        type_id,
        nullable,
        RefMode::None,
        ParameterScope::None,
        library::Concurrency::None,
    )
}

pub fn rust_type_with_scope(
    env: &Env,
    type_id: library::TypeId,
    scope: ParameterScope,
    concurrency: library::Concurrency,
) -> Result {
    rust_type_full(
        env,
        type_id,
        Nullable(false),
        RefMode::None,
        scope,
        concurrency,
    )
}

pub fn bounds_rust_type(env: &Env, type_id: library::TypeId) -> Result {
    rust_type_full(
        env,
        type_id,
        Nullable(false),
        RefMode::ByRefFake,
        ParameterScope::None,
        library::Concurrency::None,
    )
}

pub fn rust_type_full(
    env: &Env,
    type_id: library::TypeId,
    nullable: Nullable,
    ref_mode: RefMode,
    scope: ParameterScope,
    concurrency: library::Concurrency,
) -> Result {
    use crate::library::{Fundamental::*, Type::*};
    let ok = |s: &str| Ok(s.into());
    let err = |s: &str| Err(TypeError::Unimplemented(s.into()));
    let mut skip_option = false;
    let type_ = env.library.type_(type_id);
    let mut rust_type = match *type_ {
        Fundamental(fund) => {
            match fund {
                None => err("()"),
                Boolean => ok("bool"),
                Int8 => ok("i8"),
                UInt8 => ok("u8"),
                Int16 => ok("i16"),
                UInt16 => ok("u16"),
                Int32 => ok("i32"),
                UInt32 => ok("u32"),
                Int64 => ok("i64"),
                UInt64 => ok("u64"),

                Int => ok("i32"),  //maybe dependent on target system
                UInt => ok("u32"), //maybe dependent on target system

                Short => ok("libc::c_short"), //depends of target system
                UShort => ok("libc::c_ushort"), //depends of target system
                Long => ok("libc::c_long"),   //depends of target system
                ULong => ok("libc::c_ulong"), //depends of target system

                Size => ok("usize"),  //depends of target system
                SSize => ok("isize"), //depends of target system

                Float => ok("f32"),
                Double => ok("f64"),

                UniChar => ok("char"),
                Utf8 => {
                    if ref_mode.is_ref() {
                        ok("str")
                    } else {
                        ok("GString")
                    }
                }
                Filename => {
                    if ref_mode.is_ref() {
                        ok("std::path::Path")
                    } else {
                        ok("std::path::PathBuf")
                    }
                }
                OsString => {
                    if ref_mode.is_ref() {
                        ok("std::ffi::OsStr")
                    } else {
                        ok("std::ffi::OsString")
                    }
                }
                Type if env.namespaces.glib_ns_id == library::MAIN_NAMESPACE => ok("types::Type"),
                Type => ok("glib::types::Type"),
                Char if env.namespaces.glib_ns_id == library::MAIN_NAMESPACE => ok("Char"),
                Char => ok("glib::Char"),
                UChar if env.namespaces.glib_ns_id == library::MAIN_NAMESPACE => ok("UChar"),
                UChar => ok("glib::UChar"),
                Unsupported => err("Unsupported"),
                _ => err(&format!("Fundamental: {:?}", fund)),
            }
        }
        Alias(ref alias) => rust_type_full(env, alias.typ, nullable, ref_mode, scope, concurrency)
            .map_any(|_| alias.name.clone()),
        Record(library::Record { ref c_type, .. }) if c_type == "GVariantType" => {
            if ref_mode.is_ref() {
                ok("VariantTy")
            } else {
                ok("VariantType")
            }
        }
        Enumeration(..) | Bitfield(..) | Record(..) | Union(..) | Class(..) | Interface(..) => {
            let name = type_.get_name();
            if env.type_status(&type_id.full_name(&env.library)).ignored() {
                Err(TypeError::Ignored(name))
            } else {
                Ok(name)
            }
        }
        List(inner_tid) | SList(inner_tid) | CArray(inner_tid)
            if ConversionType::of(env, inner_tid) == ConversionType::Pointer =>
        {
            skip_option = true;
            let inner_ref_mode = match *env.library.type_(inner_tid) {
                Class(..) | Interface(..) => RefMode::None,
                _ => ref_mode,
            };
            rust_type_full(
                env,
                inner_tid,
                Nullable(false),
                inner_ref_mode,
                scope,
                concurrency,
            )
            .map_any(|s| {
                if ref_mode.is_ref() {
                    format!("[{}]", s)
                } else {
                    format!("Vec<{}>", s)
                }
            })
        }
        CArray(inner_tid) if ConversionType::of(env, inner_tid) == ConversionType::Direct => {
            if let Fundamental(fund) = *env.library.type_(inner_tid) {
                let array_type = match fund {
                    Int8 => Some("i8"),
                    UInt8 => Some("u8"),
                    Int16 => Some("i16"),
                    UInt16 => Some("u16"),
                    Int32 => Some("i32"),
                    UInt32 => Some("u32"),
                    Int64 => Some("i64"),
                    UInt64 => Some("u64"),

                    Int => Some("i32"),  //maybe dependent on target system
                    UInt => Some("u32"), //maybe dependent on target system

                    Float => Some("f32"),
                    Double => Some("f64"),
                    _ => Option::None,
                };

                if let Some(s) = array_type {
                    skip_option = true;
                    if ref_mode.is_ref() {
                        Ok(format!("[{}]", s))
                    } else {
                        Ok(format!("Vec<{}>", s))
                    }
                } else {
                    Err(TypeError::Unimplemented(type_.get_name()))
                }
            } else {
                Err(TypeError::Unimplemented(type_.get_name()))
            }
        }
        Custom(library::Custom { ref name, .. }) => Ok(name.clone()),
        Function(ref f) => {
            let concurrency = match concurrency {
                _ if scope.is_call() => "",
                library::Concurrency::Send | library::Concurrency::SendUnique => " + Send",
                // If an object is Sync, it can be shared between threads, and as
                // such our callback can be called from arbitrary threads and needs
                // to be Send *AND* Sync
                library::Concurrency::SendSync => " + Send + Sync",
                library::Concurrency::None => "",
            };

            let full_name = type_id.full_name(&env.library);
            if full_name == "Gio.AsyncReadyCallback" {
                return Ok(if env.namespaces.glib_ns_id == namespaces::MAIN {
                    "FnOnce(Result<(), Error>) + 'static"
                } else {
                    "FnOnce(Result<(), glib::Error>) + 'static"
                }
                .to_owned());
            } else if full_name == "GLib.DestroyNotify" {
                return Ok(format!("Fn(){} + 'static", concurrency));
            }
            let mut s = Vec::with_capacity(f.parameters.len());
            let mut err = false;
            for p in f.parameters.iter() {
                if p.closure.is_some() {
                    continue;
                }
                match rust_type_nullable(env, p.typ, p.nullable) {
                    Ok(x) => {
                        let is_fundamental = p.typ.is_fundamental_type(env);
                        let y = rust_type(env, p.typ).unwrap_or_else(|_| String::new());
                        s.push(format!(
                            "{}{}",
                            if is_fundamental || *p.nullable {
                                ""
                            } else {
                                "&"
                            },
                            if y != "GString" {
                                if !is_fundamental && *p.nullable {
                                    x.replace("Option<", "Option<&")
                                } else {
                                    x
                                }
                            } else if *p.nullable {
                                "Option<&str>".to_owned()
                            } else {
                                "&str".to_owned()
                            }
                        ));
                    }
                    e => {
                        err = true;
                        s.push(e.into_string());
                    }
                }
            }
            let closure_kind = if scope.is_call() {
                "FnMut"
            } else if scope.is_async() {
                "FnOnce"
            } else {
                "Fn"
            };
            let ret = match rust_type_nullable(env, f.ret.typ, f.ret.nullable) {
                Ok(x) => {
                    let y = rust_type(env, f.ret.typ).unwrap_or_else(|_| String::new());
                    format!(
                        "{}({}) -> {}{}",
                        closure_kind,
                        s.join(", "),
                        if y != "GString" {
                            &x
                        } else if *f.ret.nullable {
                            "Option<String>"
                        } else {
                            "String"
                        },
                        concurrency
                    )
                }
                Err(TypeError::Unimplemented(ref x)) if x == "()" => {
                    format!("{}({}){}", closure_kind, s.join(", "), concurrency)
                }
                e => {
                    err = true;
                    format!(
                        "{}({}) -> {}{}",
                        closure_kind,
                        s.join(", "),
                        e.into_string(),
                        concurrency
                    )
                }
            };
            if err {
                return Err(TypeError::Unimplemented(ret));
            }
            Ok(if *nullable {
                if scope.is_call() {
                    format!("Option<&mut dyn ({})>", ret)
                } else {
                    format!("Option<Box_<dyn {} + 'static>>", ret)
                }
            } else {
                format!("{}{}", ret, if scope.is_call() { "" } else { " + 'static" })
            })
        }
        _ => Err(TypeError::Unimplemented(type_.get_name())),
    };

    if type_id.ns_id != library::MAIN_NAMESPACE
        && type_id.ns_id != library::INTERNAL_NAMESPACE
        && type_id.full_name(&env.library) != "GLib.DestroyNotify"
        && type_id.full_name(&env.library) != "GObject.Callback"
        && !type_.is_function()
    {
        if env.type_status(&type_id.full_name(&env.library)).ignored() {
            rust_type = Err(TypeError::Ignored(into_inner(rust_type)));
        }
        rust_type = rust_type
            .map_any(|s| format!("{}::{}", env.namespaces[type_id.ns_id].higher_crate_name, s));
    }

    match ref_mode {
        RefMode::None | RefMode::ByRefFake => {}
        RefMode::ByRef | RefMode::ByRefImmut | RefMode::ByRefConst => {
            rust_type = rust_type.map_any(|s| format!("&{}", s))
        }
        RefMode::ByRefMut => rust_type = rust_type.map_any(|s| format!("&mut {}", s)),
    }
    if *nullable && !skip_option {
        match ConversionType::of(env, type_id) {
            ConversionType::Pointer | ConversionType::Scalar => {
                rust_type = rust_type.map_any(|s| format!("Option<{}>", s))
            }
            _ => (),
        }
    }

    rust_type
}

pub fn used_rust_type(env: &Env, type_id: library::TypeId, is_in: bool) -> Result {
    use crate::library::Type::*;
    match *env.library.type_(type_id) {
        Fundamental(library::Fundamental::Type)
        | Fundamental(library::Fundamental::Short)
        | Fundamental(library::Fundamental::UShort)
        | Fundamental(library::Fundamental::Long)
        | Fundamental(library::Fundamental::ULong)
        | Fundamental(library::Fundamental::Char)
        | Fundamental(library::Fundamental::UChar)
        | Fundamental(library::Fundamental::Filename)
        | Fundamental(library::Fundamental::OsString)
        | Alias(..)
        | Bitfield(..)
        | Record(..)
        | Union(..)
        | Class(..)
        | Enumeration(..)
        | Interface(..) => rust_type(env, type_id),
        //process inner types as return parameters
        List(inner_tid) | SList(inner_tid) | CArray(inner_tid) => {
            used_rust_type(env, inner_tid, false)
        }
        Custom(..) => rust_type(env, type_id),
        Fundamental(library::Fundamental::Utf8) if !is_in => Ok("::glib::GString".into()),
        _ => Err(TypeError::Ignored("Don't need use".to_owned())),
    }
}

pub fn parameter_rust_type(
    env: &Env,
    type_id: library::TypeId,
    direction: library::ParameterDirection,
    nullable: Nullable,
    ref_mode: RefMode,
    scope: ParameterScope,
) -> Result {
    use crate::library::Type::*;
    let type_ = env.library.type_(type_id);
    let rust_type = rust_type_full(
        env,
        type_id,
        nullable,
        ref_mode,
        scope,
        library::Concurrency::None,
    );
    match *type_ {
        Fundamental(fund) => {
            if (fund == library::Fundamental::Utf8
                || fund == library::Fundamental::OsString
                || fund == library::Fundamental::Filename)
                && (direction == library::ParameterDirection::InOut
                    || (direction == library::ParameterDirection::Out
                        && ref_mode == RefMode::ByRefMut))
            {
                return Err(TypeError::Unimplemented(into_inner(rust_type)));
            }
            rust_type.map_any(|s| format_parameter(s, direction))
        }

        Alias(ref alias) => rust_type
            .and_then(|s| {
                parameter_rust_type(env, alias.typ, direction, nullable, ref_mode, scope)
                    .map_any(|_| s)
            })
            .map_any(|s| format_parameter(s, direction)),

        Enumeration(..) | Union(..) | Bitfield(..) => {
            rust_type.map_any(|s| format_parameter(s, direction))
        }

        Record(..) => {
            if direction == library::ParameterDirection::InOut {
                Err(TypeError::Unimplemented(into_inner(rust_type)))
            } else {
                rust_type
            }
        }

        Class(..) | Interface(..) => match direction {
            library::ParameterDirection::In
            | library::ParameterDirection::Out
            | library::ParameterDirection::Return => rust_type,
            _ => Err(TypeError::Unimplemented(into_inner(rust_type))),
        },

        List(..) | SList(..) => match direction {
            library::ParameterDirection::In | library::ParameterDirection::Return => rust_type,
            _ => Err(TypeError::Unimplemented(into_inner(rust_type))),
        },
        CArray(..) => match direction {
            library::ParameterDirection::In
            | library::ParameterDirection::Out
            | library::ParameterDirection::Return => rust_type,
            _ => Err(TypeError::Unimplemented(into_inner(rust_type))),
        },
        Function(ref func) if func.name == "AsyncReadyCallback" => {
            Ok("AsyncReadyCallback".to_string())
        }
        Function(_) => rust_type,
        Custom(..) => rust_type.map_any(|s| format_parameter(s, direction)),
        _ => Err(TypeError::Unimplemented(type_.get_name())),
    }
}

#[inline]
fn format_parameter(rust_type: String, direction: library::ParameterDirection) -> String {
    if direction.is_out() {
        format!("&mut {}", rust_type)
    } else {
        rust_type
    }
}
