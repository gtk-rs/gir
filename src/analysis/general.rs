use gobjects::*;
use library::*;

pub struct StatusedTypeId{
    pub type_id: TypeId,
    pub name: String,
    pub status: GStatus,
}

pub trait IsChildOfSpecialType {
    fn is_child_of_special_type(&self) -> bool;
}

impl IsChildOfSpecialType for Class {
    fn is_child_of_special_type(&self) -> bool {
        self.parents.contains(&SPECIAL_TYPE_ID)
    }
}

pub fn is_child_of_special_type(name: &str, library: &Library) -> bool {
    match library.type_(library.find_type_unwrapped(0, name, "Type")) {
        &Type::Class(ref klass) => klass.is_child_of_special_type(),
        _ => false,
    }
}
