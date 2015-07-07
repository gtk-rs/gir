use library;

pub fn needed_upcast(library: &library::Library, type_id: library::TypeId) -> bool {
    match library.type_(type_id) {
        &library::Type::Class(ref klass) => !klass.children.is_empty(),
        _ => false,
    }
}
