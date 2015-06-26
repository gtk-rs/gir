use gobjects::*;
use library::*;

pub struct StatusedTypeId{
    pub type_id: TypeId,
    pub name: String,
    pub status: GStatus,
}

pub fn widget_tid(library: &Library) -> TypeId {
    library.find_type(0, "Gtk.Widget").unwrap_or_else(|| unreachable!())
}

pub trait IsWidget {
    fn is_widget(&self, library: &Library) -> bool;
}

impl IsWidget for Class {
    fn is_widget(&self, library: &Library) -> bool {
        self.glib_type_name == "GtkWidget" || self.parents.contains(&widget_tid(&library))
    }
}

impl IsWidget for Type {
    fn is_widget(&self, library: &Library) -> bool {
        match self {
            &Type::Class(ref klass) => klass.is_widget(&library),
            _ => false,
        }
    }
}

impl IsWidget for TypeId {
    fn is_widget(&self, library: &Library) -> bool {
        library.type_(*self).is_widget(&library)
    }
}

impl IsWidget for String {
    fn is_widget(&self, library: &Library) -> bool {
        library.find_type(0, self).unwrap().is_widget(library)
    }
}
