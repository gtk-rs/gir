use std::result;

use library;
use nameutil::module_name;

pub type Result = result::Result<String, String>;

pub trait AsStr {
    fn as_str(&self) -> &str;
}

impl AsStr for Result {
    #[inline]
    fn as_str(&self) -> &str {
        self.as_ref().unwrap_or_else(|s| s)
    }
}

pub fn rust_type(library: &library::Library, type_id: library::TypeId) -> Result {
    use library::Type::*;
    use library::Fundamental::*;
    let type_ = library.type_(type_id);
    let rust_type = match type_ {
        &Fundamental(fund) => {
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

                Utf8 => ok("String"),

                Type => ok("types::Type"),
                Unsupported => err("Unsupported"),
                _ => err(&format!("Fundamental: {:?}", fund)),
            }
        },

        &Enumeration(ref enum_) => Ok(enum_.name.clone()),
        &Interface(ref interface) => Ok(interface.name.clone()),
        &Class(ref klass) => Ok(klass.name.clone()),
        _ => Err(format!("Unknown rust type: {:?}", type_.get_name())),
        //TODO: check usage library::Type::get_name() when no _ in this
    };
    if type_id.ns_id == library::MAIN_NAMESPACE || type_id.ns_id == library::INTERNAL_NAMESPACE {
        rust_type
    } else {
        rust_type.map(|s| format!("{}::{}",
            module_name(&library.namespace(type_id.ns_id).name), s))
    }
}

pub fn used_rust_type(library: &library::Library, type_id: library::TypeId) -> Result {
    use library::Type::*;
    match library.type_(type_id) {
        &Enumeration(_) |
            &Interface(_) |
            &Class(_) => rust_type(library, type_id),
        _ => Err("Don't need use".into()),
    }
}

pub fn parameter_rust_type(library: &library::Library, type_id:library::TypeId, direction: library::ParameterDirection) -> Result {
    use library::Type::*;
    let type_ = library.type_(type_id);
    let rust_type = rust_type(library, type_id);
    match type_ {
        &Fundamental(fund) => {
            if fund == library::Fundamental::Utf8 {
                match direction {
                    library::ParameterDirection::In => Ok("&str".into()),
                    library::ParameterDirection::Return => rust_type,
                    _ => Err(format!("/*Unimplemented*/{}", rust_type.as_str())),
                }
            } else {
                format_parameter(rust_type, direction)
            }
        },

        &Enumeration(_) => format_parameter(rust_type, direction),

        &Class(_) => {
            match direction {
                library::ParameterDirection::In => rust_type.map(|s| format!("&{}", s)),
                library::ParameterDirection::Return => rust_type,
                _ => Err(format!("/*Unimplemented*/{}", rust_type.as_str())),
            }
        },
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
