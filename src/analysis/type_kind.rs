use library::*;
use super::general::IsChildOfSpecialType;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Direct,     //coded without conversion
    Converted,  //coded with from_glib
    Pointer,    //coded with from_glib_xxx
    Object,     //coded with from_glib_xxx
    Interface,  //coded with from_glib_xxx
    SpecialType,//coded with <SpecialTypeRustName>::from_glib_xxx
    Enumeration,//coded without conversion
    Unknown,
}

impl Default for TypeKind {
    fn default() -> Self { TypeKind::Unknown }
}

impl TypeKind {
    pub fn of(library: &Library, type_id: TypeId) -> TypeKind {
        use library::Type::*;
        use library::Fundamental::*;
        if type_id == SPECIAL_TYPE_ID { return TypeKind::SpecialType };
        match library.type_(type_id) {
            &Fundamental(fund) => match fund {
                Boolean => TypeKind::Converted,
                Int8 => TypeKind::Direct,
                UInt8 => TypeKind::Direct,
                Int16 => TypeKind::Direct,
                UInt16 => TypeKind::Direct,
                Int32 => TypeKind::Direct,
                UInt32 => TypeKind::Direct,
                Int64 => TypeKind::Direct,
                UInt64 => TypeKind::Direct,
                Char => TypeKind::Converted,
                UChar => TypeKind::Converted,
                Int => TypeKind::Direct,
                UInt => TypeKind::Direct,
                Long => TypeKind::Direct,
                ULong => TypeKind::Direct,
                Size => TypeKind::Direct,
                SSize => TypeKind::Direct,
                Float => TypeKind::Direct,
                Double => TypeKind::Direct,
                UniChar => TypeKind::Unknown,
                Pointer => TypeKind::Pointer,
                VarArgs => TypeKind::Unknown,
                Utf8 => TypeKind::Pointer,
                Filename => TypeKind::Pointer,
                Type => TypeKind::Enumeration,
                None => TypeKind::Unknown,
                Unsupported => TypeKind::Unknown,
            },
            &Enumeration(_) => TypeKind::Enumeration,
            &Interface(_) => TypeKind::Interface,
            &Class(ref klass) => {
                if klass.is_child_of_special_type() { TypeKind::SpecialType } else { TypeKind::Object }
            },
            _ => TypeKind::Unknown,
        }
    }
}
