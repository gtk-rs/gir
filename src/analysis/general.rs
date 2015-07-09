use gobjects::*;
use library::*;

pub struct StatusedTypeId{
    pub type_id: TypeId,
    pub name: String,
    pub status: GStatus,
}

fn widget_tid(library: &Library) -> TypeId {
    library.find_type(0, "Gtk.Widget").expect("type Gtk.Widget not found")
}

pub fn is_widget(name: &str, library: &Library) -> bool {
    match library.type_(library.find_type_unwrapped(0, name, "Type")) {
        &Type::Class(ref klass) => klass.parents
            .contains(&widget_tid(&library)),
        _ => false,
    }
}
