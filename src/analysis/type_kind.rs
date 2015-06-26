use library::*;
use super::general::IsWidget;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Simply,     //coded with from_glib
    Pointer,    //coded with from_glib_xxx
    Object,
    Interface,
    Widget,     //coded with Widget::from_glib_xxx
    Unknown,
}

impl Default for TypeKind {
    fn default() -> Self { TypeKind::Unknown }
}

pub trait ToTypeKind {
    fn to_type_kind(&self, library: &Library) -> TypeKind;
}

impl ToTypeKind for Type {
    fn to_type_kind(&self, library: &Library) -> TypeKind {
        match self {
            &Type::Fundamental(_) => TypeKind::Simply,
            &Type::Interface(_) => TypeKind::Interface,
            &Type::Class(ref klass) => {
                if klass.is_widget(&library) { TypeKind::Widget } else { TypeKind::Object }
            },
            _ => TypeKind::Unknown,
        }
    }
}
