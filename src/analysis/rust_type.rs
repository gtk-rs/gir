use std::result;

use analysis::ref_mode::RefMode;
use env::Env;
use library::{self, Nullable};
use nameutil::crate_name;
use super::conversion_type::ConversionType;
use traits::*;

pub type Result = result::Result<String, String>;

impl AsStr for Result {
    #[inline]
    fn as_str(&self) -> &str {
        self.as_ref().unwrap_or_else(|s| s)
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
    let mut skip_option = false;
    let type_ = env.library.type_(type_id);
    let mut rust_type = match *type_ {
        Fundamental(fund) => {
            let ok = |s: &str| Ok(s.into());
            let err = |s: &str| Err(s.into());
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
                Filename => if ref_mode.is_ref() { ok("std::path::Path") } else { ok("std::path::PathBuf") },

                Type => ok("glib::types::Type"),
                Unsupported => err("Unsupported"),
                _ => err(&format!("Fundamental: {:?}", fund)),
            }
        },
        Alias(ref alias) => rust_type_full(env, alias.typ, nullable, ref_mode)
                .map_any(|_| alias.name.clone()),

        Enumeration(ref enum_) => Ok(enum_.name.clone()),
        Bitfield(ref bitfield) => Ok(bitfield.name.clone()),
        Record(ref record) => Ok(record.name.clone()),
        Interface(ref interface) => Ok(interface.name.clone()),
        Class(ref klass) => Ok(klass.name.clone()),
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
                    format!("[{}]", s)
                } else {
                    format!("Vec<{}>", s)
                })
        }
        _ => Err(format!("Unknown rust type: {:?}", type_.get_name())),
        //TODO: check usage library::Type::get_name() when no _ in this
    };

    if type_id.ns_id != library::MAIN_NAMESPACE && type_id.ns_id != library::INTERNAL_NAMESPACE
        && !implemented_in_main_namespace(&env.library, type_id) {
        let rust_type_with_prefix = rust_type.map(|s| format!("{}::{}",
            crate_name(&env.library.namespace(type_id.ns_id).name), s));
        if env.type_status(&type_id.full_name(&env.library)).ignored() {
            rust_type = Err(format!("/*Ignored*/{}", rust_type_with_prefix.as_str()));
        } else {
            rust_type = rust_type_with_prefix;
        }
    }
    match ref_mode {
        RefMode::None | RefMode::ByRefFake => {}
        RefMode::ByRef | RefMode::ByRefImmut => rust_type = rust_type.map_any(|s| format!("&{}", s)),
        RefMode::ByRefMut => rust_type = rust_type.map_any(|s| format!("&mut {}", s)),
    }
    if *nullable && !skip_option {
        match ConversionType::of(&env.library, type_id) {
            ConversionType::Pointer
                | ConversionType::Scalar => {
                rust_type = rust_type.map_any(|s| format!("Option<{}>", s))
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
        _ => Err("Don't need use".into()),
    }
}

pub fn parameter_rust_type(env: &Env, type_id:library::TypeId,
                           direction: library::ParameterDirection, nullable: Nullable,
                           ref_mode: RefMode) -> Result {
    use library::Type::*;
    let type_ = env.library.type_(type_id);
    let rust_type = rust_type_full(env, type_id, nullable, ref_mode);
    match *type_ {
        Fundamental(fund) => {
            if fund == library::Fundamental::Utf8 || fund == library::Fundamental::Filename {
                match direction {
                    library::ParameterDirection::In |
                        library::ParameterDirection::Return => rust_type,
                    _ => Err(format!("/*Unimplemented*/{}", rust_type.as_str())),
                }
            } else {
                format_parameter(rust_type, direction)
            }
        }
        Alias(ref alias) => {
            let res = format_parameter(rust_type, direction);
            if parameter_rust_type(env, alias.typ, direction, nullable, ref_mode).is_ok() {
                res
            } else {
                res.and_then(|s| Err(s))
            }
        }

        Enumeration(..) |
            Bitfield(..) => format_parameter(rust_type, direction),

        Record(..) => {
            if direction == library::ParameterDirection::InOut {
                Err(format!("/*Unimplemented*/{}", rust_type.as_str()))
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
                _ => Err(format!("/*Unimplemented*/{}", rust_type.as_str())),
            }
        }

        List(..) |
            SList(..) |
            CArray(..) => {
            match direction {
                library::ParameterDirection::In |
                    library::ParameterDirection::Return => rust_type,
                _ => Err(format!("/*Unimplemented*/{}", rust_type.as_str())),
            }
        }
        _ => Err(format!("Unknown rust type: {:?}", type_.get_name())),
        //TODO: check usage library::Type::get_name() when no _ in this
    }
}

#[inline]
fn format_parameter(rust_type: Result, direction: library::ParameterDirection) -> Result {
    if direction.is_out() {
        rust_type.map(|s| format!("&mut {}", s))
    } else {
        rust_type
    }
}

fn implemented_in_main_namespace(library: &library::Library, type_id: library::TypeId) -> bool {
    if library.namespace(library::MAIN_NAMESPACE).name != "Gtk" {
        return false;
    }
    match &*type_id.full_name(library) {
        "Gdk.Rectangle" => true,
        "GLib.Error" => true,
        _ => false,
    }
}
