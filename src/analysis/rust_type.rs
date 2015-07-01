use library;

pub trait ToRustType {
    fn to_rust_type(&self) -> String;
}

impl ToRustType for library::Fundamental {
    fn to_rust_type(&self) -> String {
        use library::Fundamental::*;
        match self {
            &Boolean => "bool".into(),
            &Int8 => "i8".into(),
            &UInt8 => "u8".into(),
            &Int16 => "i16".into(),
            &UInt16 => "u16".into(),
            &Int32 => "i32".into(),
            &UInt32 => "u32".into(),
            &Int64 => "i64".into(),
            &UInt64 => "u64".into(),

            &Int => "i32".into(),      //maybe dependent on target system
            &UInt => "i32".into(),     //maybe dependent on target system

            &Type => "Type".into(),
            &Unsupported => "Unsupported".into(),
            _ => format!("Fundamental: {:?}", self),
        }
    }
}

impl ToRustType for library::Type {
    fn to_rust_type(&self) -> String {
        use library::Type::*;
        match self {
            &Fundamental(fund) => fund.to_rust_type(),

            &Enumeration(ref enum_) => enum_.name.clone(),

            &Interface(ref interface) => interface.name.clone(),
            &Class(ref class) => class.name.clone(),
            _ => format!("Unknown rust type: {:?}", self.get_name()),
            //TODO: check usage library::Type::get_name() when no _ in this
        }
    }
}
