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
            //TODO: Rework out (without inout) parameters as return type, with checking that it last
            //  Ex. gtk_range_get_range_rect, gtk_scale_get_layout_offsets
            //TODO: change Utf8 to &str for inputs
            let type_ = library.type_(self.typ);
            let type_name = type_.to_rust_type();
            let kind = type_.to_type_kind(library);
            let mut type_str = match kind {
                TypeKind::Unknown => format!("/*Unknown kind*/{}", type_name),
                TypeKind::Simple |
                    TypeKind::Enumeration => if self.direction.is_out() {
                        format!("&mut {}", type_name)
                    } else { type_name },

                _ => if self.direction.is_out() {
                        panic!("Out parameter '{}' for TypeKind::{:?} ", self.name, kind)
                        //format!("&mut {}", type_name)
                    } else {
                        format!("&{}", type_name)
                    },
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
