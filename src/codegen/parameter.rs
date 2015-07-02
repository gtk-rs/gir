use library;
use analysis::type_kind::{TypeKind, ToTypeKind};
use analysis::rust_type::ToRustType;

pub trait ToParameter {
    fn to_parameter(&self, library: &library::Library) -> String;
}

impl ToParameter for library::Parameter {
    fn to_parameter(&self, library: &library::Library) -> String {
        if self.instance_parameter {
            "&self".into()
        } else {
            //TODO: Out parameters. Ex. gtk_range_get_range_rect
            //TODO: change Utf8 to &str for inputs
            let type_ = library.type_(self.typ);
            let type_name = type_.to_rust_type();
            let kind = type_.to_type_kind(library);
            let mut type_str = match kind {
                TypeKind::Unknown => format!("/*Unknown kind*/{}", type_name),
                TypeKind::Simple |
                    TypeKind::Enumeration => type_name,

                _ => format!("&{}", type_name),
            };
            if self.nullable {
                type_str = format!("Option<{}>", type_str);
            }
            format_parameter(&self.name, &type_str)
        }
    }
}

fn format_parameter(name: &str, type_str: &str) -> String {
    format!("{}: {}", name, type_str)
}
