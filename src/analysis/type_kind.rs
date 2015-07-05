use library::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Direct,     //coded without conversion
    Converted,  //coded with from_glib
    Pointer,    //coded with from_glib_xxx
    Object,     //coded with from_glib_xxx
    Interface,  //coded with from_glib_xxx
    Enumeration,//coded without conversion
    Unknown,
}

impl Default for TypeKind {
    fn default() -> Self { TypeKind::Unknown }
}

pub trait ToTypeKind {
    fn to_type_kind(&self, library: &Library) -> TypeKind;
}

impl ToTypeKind for Fundamental {
    fn to_type_kind(&self, _library: &Library) -> TypeKind {
        use library::Fundamental::*;
        match self {
            &Boolean => TypeKind::Converted,
            &Int8 => TypeKind::Direct,
            &UInt8 => TypeKind::Direct,
            &Int16 => TypeKind::Direct,
            &UInt16 => TypeKind::Direct,
            &Int32 => TypeKind::Direct,
            &UInt32 => TypeKind::Direct,
            &Int64 => TypeKind::Direct,
            &UInt64 => TypeKind::Direct,
            &Char => TypeKind::Converted,
            &UChar => TypeKind::Converted,
            &Int => TypeKind::Direct,
            &UInt => TypeKind::Direct,
            &Long => TypeKind::Direct,
            &ULong => TypeKind::Direct,
            &Size => TypeKind::Direct,
            &SSize => TypeKind::Direct,
            &Float => TypeKind::Direct,
            &Double => TypeKind::Direct,
            &UniChar => TypeKind::Unknown,
            &Pointer => TypeKind::Pointer,
            &VarArgs => TypeKind::Unknown,
            &Utf8 => TypeKind::Pointer,
            &Filename => TypeKind::Pointer,
            &Type => TypeKind::Enumeration,
            &None => TypeKind::Unknown,
            &Unsupported => TypeKind::Unknown,
        }
    }
}

impl ToTypeKind for Type {
    fn to_type_kind(&self, library: &Library) -> TypeKind {
        match self {
            &Type::Fundamental(fund) => fund.to_type_kind(library),
            &Type::Enumeration(_) => TypeKind::Enumeration,
            &Type::Interface(_) => TypeKind::Interface,
            &Type::Class(_) => TypeKind::Object,
            _ => TypeKind::Unknown,
        }
    }
}
