use library::{Library, Type, TypeId};

pub fn needed_upcast(library: &Library, type_id: TypeId) -> bool {
    match *library.type_(type_id) {
        Type::Class(ref klass) => !klass.children.is_empty(),
        Type::Interface(..) => true,
        _ => false,
    }
}
