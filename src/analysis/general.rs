use gobjects::*;
use library::*;

pub struct StatusedTypeId{
    pub type_id: TypeId,
    pub name: String,
    pub status: GStatus,
}

pub trait IsSpecialType {
    fn is_special_type(&self) -> bool;
}

impl IsSpecialType for Class {
    fn is_special_type(&self) -> bool {
        self.glib_type_name == "GtkWidget" || self.parents.contains(&SPECIAL_TYPE_ID)
    }
}

pub fn is_special_type(name: &str, library: &Library) -> bool {
    match library.type_(library.find_type_unwrapped(0, name, "Type")) {
        &Type::Class(ref klass) => klass.is_special_type(),
        _ => false,
    }
}
