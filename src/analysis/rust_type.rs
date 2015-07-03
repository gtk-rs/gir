use std::result;

use library;

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

pub trait ToRustType {
    fn to_rust_type(&self) -> Result;
}

impl ToRustType for library::Fundamental {
    fn to_rust_type(&self) -> Result {
        use library::Fundamental::*;
        let ok = |s: &str| Ok(s.into());
        let err = |s: &str| Err(s.into());
        match self {
            &None => err("()"),
            &Boolean => ok("bool"),
            &Int8 => ok("i8"),
            &UInt8 => ok("u8"),
            &Int16 => ok("i16"),
            &UInt16 => ok("u16"),
            &Int32 => ok("i32"),
            &UInt32 => ok("u32"),
            &Int64 => ok("i64"),
            &UInt64 => ok("u64"),

            &Int => ok("i32"),      //maybe dependent on target system
            &UInt => ok("i32"),     //maybe dependent on target system

            &Float => ok("f32"),
            &Double => ok("f64"),

            &Utf8 => ok("String"),

            &Type => ok("Type"),
            &Unsupported => err("Unsupported"),
            _ => err(&format!("Fundamental: {:?}", self)),
        }
    }
}

impl ToRustType for library::Enumeration {
    fn to_rust_type(&self) -> Result {
        Ok(self.name.clone())
    }
}

impl ToRustType for library::Interface {
    fn to_rust_type(&self) -> Result {
        Ok(self.name.clone())
    }
}

impl ToRustType for library::Class {
    fn to_rust_type(&self) -> Result {
        Ok(self.name.clone())
    }
}

impl ToRustType for library::Type {
    fn to_rust_type(&self) -> Result {
        use library::Type::*;
        match self {
            &Fundamental(fund) => fund.to_rust_type(),

            &Enumeration(ref enum_) => enum_.to_rust_type(),

            &Interface(ref interface) => interface.to_rust_type(),
            &Class(ref class) => class.to_rust_type(),
            _ => Err(format!("Unknown rust type: {:?}", self.get_name())),
            //TODO: check usage library::Type::get_name() when no _ in this
        }
    }
}

pub trait ToParameterRustType {
    fn to_parameter_rust_type(&self, direction: library::ParameterDirection) -> Result;
}

impl ToParameterRustType for library::Fundamental {
    fn to_parameter_rust_type(&self, direction: library::ParameterDirection) -> Result {
        let rust_type = self.to_rust_type();
        if self == &library::Fundamental::Utf8 {
            return match direction {
                library::ParameterDirection::In => Ok("&str".into()),
                library::ParameterDirection::Return => rust_type,
                _ => Err(format!("/*Unimplemented*/{}", rust_type.as_str())),
            }
        }
        if direction.is_out() {
            rust_type.map(|s| format!("&mut {}", s))
        } else {
            rust_type
        }
    }
}

impl ToParameterRustType for library::Enumeration {
    fn to_parameter_rust_type(&self, direction: library::ParameterDirection) -> Result {
        let rust_type = self.to_rust_type();
        if direction.is_out() {
            rust_type.map(|s| format!("&mut {}", s))
        } else {
            rust_type
        }
    }
}

impl ToParameterRustType for library::Class {
    fn to_parameter_rust_type(&self, direction: library::ParameterDirection) -> Result {
        let rust_type = self.to_rust_type();
        match direction {
            library::ParameterDirection::In => rust_type.map(|s| format!("&{}", s)),
            library::ParameterDirection::Return => rust_type,
            _ => Err(format!("/*Unimplemented*/{}", rust_type.as_str())),
        }
    }
}

impl ToParameterRustType for library::Type {
    fn to_parameter_rust_type(&self, direction: library::ParameterDirection) -> Result {
        use library::Type::*;
        match self {
            &Fundamental(fund) => fund.to_parameter_rust_type(direction),

            &Enumeration(ref enum_) => enum_.to_parameter_rust_type(direction),

            //TODO: &Interface(ref interface) => interface.to_parameter_rust_type(direction),
            &Class(ref class) => class.to_parameter_rust_type(direction),
            _ => Err(format!("Unknown rust type: {:?}", self.get_name())),
            //TODO: check usage library::Type::get_name() when no _ in this
        }
    }
}
