use analysis::type_kind::TypeKind;
use library;

pub trait TranslateToGlib {
    fn translate_to_glib(&self, library: &library::Library, in_trait: bool) -> String;
}

impl TranslateToGlib for library::Parameter {
    fn translate_to_glib(&self, library: &library::Library, in_trait: bool) -> String {
        if self.instance_parameter {
            let upcast_str = if in_trait { ".upcast()" } else { "" };
            format!("self{}.to_glib_none().0", upcast_str)
        } else {
            let kind = TypeKind::of(library, self.typ);
            match kind {
                TypeKind::Converted => format_parameter(&self.name, "to_glib()"),
                TypeKind::Pointer => format_parameter(&self.name, "to_glib_none().0"),
                TypeKind::Direct |
                    TypeKind::Enumeration => self.name.clone(),
                TypeKind::Object => to_glib_xxx(&self.name, self.transfer),
                _ => format!("TODO:{}", self.name)
            }
        }
    }
}

fn format_parameter(name: &str, convert: &str) -> String {
    format!("{}.{}", name, convert)
}

fn to_glib_xxx(name: &str, transfer: library::Transfer) -> String {
    use library::Transfer::*;
    match transfer {
        None => format_parameter(name, "to_glib_none().0"),
        Full => format_parameter(name, "to_glib_full().0"),
        Container => format!("TODO:container {}", name),
    }
}
