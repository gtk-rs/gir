use analysis::type_kind::TypeKind;
use library;

pub trait TranslateToGlib {
    fn translate_to_glib(&self, library: &library::Library, upcast: bool) -> String;
}

impl TranslateToGlib for library::Parameter {
    fn translate_to_glib(&self, library: &library::Library, upcast: bool) -> String {
        let kind = TypeKind::of(library, self.typ);
        let upcast_str = match (upcast, *self.nullable) {
            (true, true) => ".map(Upcast::upcast)",
            (true, false) => ".upcast()",
            _ => "",
        };
        match kind {
            TypeKind::Converted => format!("{}{}", self.name, ".to_glib()"),
            TypeKind::Direct |
                TypeKind::Bitfield |
                TypeKind::Enumeration => self.name.clone(),
            TypeKind::Pointer |
                TypeKind::Container |
                TypeKind::Object => {
                if self.instance_parameter {
                    format!("self{}{}", upcast_str, to_glib_xxx(self.transfer))
                }
                else {
                    format!("{}{}{}", self.name, upcast_str, to_glib_xxx(self.transfer))
                }
            }
            _ => format!("TODO:{}", self.name)
        }
    }
}

fn to_glib_xxx(transfer: library::Transfer) -> &'static str {
    use library::Transfer::*;
    match transfer {
        None => ".to_glib_none().0",
        Full => ".to_glib_full()",
        Container => ".to_glib_container()",
    }
}
