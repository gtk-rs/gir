use library::*;
use super::general::IsWidget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Simple,     //coded with from_glib
    Pointer,    //coded with from_glib_xxx
    Object,
    Interface,
    Widget,     //coded with Widget::from_glib_xxx
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
            &Boolean => TypeKind::Simple,
            &Int8 => TypeKind::Simple,
            &UInt8 => TypeKind::Simple,
            &Int16 => TypeKind::Simple,
            &UInt16 => TypeKind::Simple,
            &Int32 => TypeKind::Simple,
            &UInt32 => TypeKind::Simple,
            &Int64 => TypeKind::Simple,
            &UInt64 => TypeKind::Simple,
            &Char => TypeKind::Simple,
            &UChar => TypeKind::Simple,
            &Int => TypeKind::Simple,
            &UInt => TypeKind::Simple,
            &Long => TypeKind::Simple,
            &ULong => TypeKind::Simple,
            &Size => TypeKind::Simple,
            &SSize => TypeKind::Simple,
            &Float => TypeKind::Simple,
            &Double => TypeKind::Simple,
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
            &Type::Class(ref klass) => {
                if klass.is_widget(&library) { TypeKind::Widget } else { TypeKind::Object }
            },
            _ => TypeKind::Unknown,
        }
    }
}
