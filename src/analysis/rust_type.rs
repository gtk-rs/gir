use std::result;

use analysis::ref_mode::RefMode;
use env::Env;
use library::{self, Nullable};
use super::conversion_type::ConversionType;
use traits::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeError<'a> {
    Ignored(Cow<'a, str>),
    Mismatch(Cow<'a, str>),
    Unimplemented(Cow<'a, str>),
}

pub type Result<'a> = result::Result<Cow<'a, str>, TypeError<'a>>;

fn into_inner(res: Result) -> Cow<str> {
    use self::TypeError::*;
    match res {
        Ok(s) |
        Err(Ignored(s)) |
        Err(Mismatch(s)) |
        Err(Unimplemented(s)) => s,
    }
}

impl<'a> IntoString for Result<'a> {
    fn into_string(self) -> String {
        use self::TypeError::*;
        match self {
            Ok(s) => s.into_owned(),
            Err(Ignored(s)) => format!("/*Ignored*/{}", s),
            Err(Mismatch(s)) => format!("/*Metadata mismatch*/{}", s),
            Err(Unimplemented(s)) => format!("/*Unimplemented*/{}", s),
        }
    }
}

impl<'a> ToCowStr for Result<'a> {
    fn to_cow_str(&self) -> Cow<str> {
        use self::TypeError::*;
        match *self {
            Ok(ref s) => s.clone(),
            Err(Ignored(ref s)) => format!("/*Ignored*/{}", s).into(),
            Err(Mismatch(ref s)) => format!("/*Metadata mismatch*/{}", s).into(),
            Err(Unimplemented(ref s)) => format!("/*Unimplemented*/{}", s).into(),
        }
    }
}

impl<'a> MapAny<'a, str> for Result<'a>  {
    fn map_any<F: FnOnce(Cow<'a, str>) -> Cow<'a, str>>(self, op: F) -> Self {
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
    rust_type_full(env, type_id, Nullable(false), RefMode::None)
}

pub fn bounds_rust_type(env: &Env, type_id: library::TypeId) -> Result {
    rust_type_full(env, type_id, Nullable(false), RefMode::ByRefFake)
}

fn rust_type_full(env: &Env, type_id: library::TypeId, nullable: Nullable, ref_mode: RefMode) -> Result {
    use library::Type::*;
    use library::Fundamental::*;
    let ok = |s: &'static str| Ok(s.into());
    let err = |s: &'static str| Err(TypeError::Unimplemented(s.into()));
    let err_owned = |s: String| Err(TypeError::Unimplemented(s.into()));
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

                Int => ok("i32"),      //maybe dependent on target system
                UInt => ok("u32"),     //maybe dependent on target system

                Float => ok("f32"),
                Double => ok("f64"),

                UniChar => ok("char"),
                Utf8 => if ref_mode.is_ref() { ok("str") } else { ok("String") },
                Filename => {
                    if ref_mode.is_ref() {
                        ok("std::path::Path")
                    }
                    else {
                        ok("std::path::PathBuf")
                    }
                }
                Type => ok("glib::types::Type"),
                Unsupported => err("Unsupported"),
                _ => err_owned(format!("Fundamental: {:?}", fund)),
            }
        },
        Alias(ref alias) => {
            rust_type_full(env, alias.typ, nullable, ref_mode)
                .map_any(|_| Cow::Borrowed(&*alias.name))
        }
        Record(library::Record { ref c_type, .. }) if c_type == "GVariantType" => {
            if ref_mode.is_ref() { ok("VariantTy") } else { ok("VariantType") }
        }
        Enumeration(..) |
            Bitfield(..) |
            Record(..) |
            Class(..) |
            Interface(..) => {
            let name = type_.get_name();
            if env.type_status(&type_id.full_name(&env.library)).ignored() {
                Err(TypeError::Ignored(name))
            }
            else {
                Ok(name)
            }
        }
        List(inner_tid) |
            SList(inner_tid) |
            CArray(inner_tid)
            if ConversionType::of(&env.library, inner_tid) == ConversionType::Pointer => {
            skip_option = true;
            let inner_ref_mode = match *env.library.type_(inner_tid) {
                Class(..) |
                    Interface(..) => RefMode::None,
                _ => ref_mode,
            };
            rust_type_full(env, inner_tid, Nullable(false), inner_ref_mode)
                .map_any(|s| if ref_mode.is_ref() {
                    format!("[{}]", s).into()
                } else {
                    format!("Vec<{}>", s).into()
                })
        }
        _ => Err(TypeError::Unimplemented(type_.get_name())),
    };

    if type_id.ns_id != library::MAIN_NAMESPACE && type_id.ns_id != library::INTERNAL_NAMESPACE
        && !implemented_in_main_namespace(&env.library, type_id) {
        if env.type_status(&type_id.full_name(&env.library)).ignored() {
            rust_type = Err(TypeError::Ignored(into_inner(rust_type)));
        }
        rust_type = rust_type.map_any(|s| format!("{}::{}",
            env.namespaces[type_id.ns_id].higher_crate_name, s).into());
    }

    match ref_mode {
        RefMode::None | RefMode::ByRefFake => {}
        RefMode::ByRef | RefMode::ByRefImmut =>
            rust_type = rust_type.map_any(|s| format!("&{}", s).into()),
        RefMode::ByRefMut => rust_type = rust_type.map_any(|s| format!("&mut {}", s).into()),
    }
    if *nullable && !skip_option {
        match ConversionType::of(&env.library, type_id) {
            ConversionType::Pointer
                | ConversionType::Scalar => {
                rust_type = rust_type.map_any(|s| format!("Option<{}>", s).into())
            }
            _ => (),
        }
    }

    rust_type
}

pub fn used_rust_type(env: &Env, type_id: library::TypeId) -> Result {
    use library::Type::*;
    match *env.library.type_(type_id) {
        Fundamental(library::Fundamental::Type) |
            Alias(..) |
            Bitfield(..) |
            Record(..) |
            Class(..) |
            Enumeration(..) |
            Interface(..) => rust_type(env, type_id),
        List(inner_tid) |
            SList(inner_tid) |
            CArray(inner_tid) => used_rust_type(env, inner_tid),
        _ => Err(TypeError::Ignored("Don't need use".into())),
    }
}

pub fn parameter_rust_type<'e>(env: &'e Env, type_id:library::TypeId,
                           direction: library::ParameterDirection, nullable: Nullable,
                           ref_mode: RefMode) -> Result<'e> {
    use library::Type::*;
    let type_ = env.library.type_(type_id);
    let rust_type = rust_type_full(env, type_id, nullable, ref_mode);
    match *type_ {
        Fundamental(fund) => {
            if fund == library::Fundamental::Utf8 || fund == library::Fundamental::Filename {
                match direction {
                    library::ParameterDirection::In |
                        library::ParameterDirection::Return => rust_type,
                    _ => Err(TypeError::Unimplemented(into_inner(rust_type))),
                }
            } else {
                rust_type.map_any(|s| format_parameter(s, direction))
            }
        }
        Alias(ref alias) => {
            rust_type.and_then(|s| {
                parameter_rust_type(env, alias.typ, direction, nullable, ref_mode)
                    .map_any(|_| s)
            })
                .map_any(|s| format_parameter(s, direction))
        }

        Enumeration(..) |
            Bitfield(..) => rust_type.map_any(|s| format_parameter(s, direction)),

        Record(..) => {
            if direction == library::ParameterDirection::InOut {
                Err(TypeError::Unimplemented(into_inner(rust_type)))
            } else {
                rust_type
            }
        }

        Class(..) |
            Interface (..) => {
            match direction {
                library::ParameterDirection::In |
                    library::ParameterDirection::Out |
                    library::ParameterDirection::Return => rust_type,
                _ => Err(TypeError::Unimplemented(into_inner(rust_type))),
            }
        }

        List(..) |
            SList(..) |
            CArray(..) => {
            match direction {
                library::ParameterDirection::In |
                    library::ParameterDirection::Return => rust_type,
                _ => Err(TypeError::Unimplemented(into_inner(rust_type))),
            }
        }
        _ => Err(TypeError::Unimplemented(type_.get_name())),
    }
}

#[inline]
fn format_parameter(rust_type: Cow<str>, direction: library::ParameterDirection) -> Cow<str> {
    if direction.is_out() {
        format!("&mut {}", rust_type).into()
    } else {
        rust_type
    }
}

fn implemented_in_main_namespace(library: &library::Library, type_id: library::TypeId) -> bool {
    match &*type_id.full_name(library) {
        "GLib.Error" => return true,
        _ => (),
    }
    if library.namespace(library::MAIN_NAMESPACE).name != "Gtk" {
        return false;
    }
    match &*type_id.full_name(library) {
        "Gdk.Rectangle" => true,
        _ => false,
    }
}
