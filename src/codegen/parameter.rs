use library;
use analysis::type_kind::TypeKind;
use analysis::rust_type::{AsStr, parameter_rust_type};
use analysis::upcasts::Upcasts;

pub trait ToParameter {
    fn to_parameter(&self, library: &library::Library, upcasts: &Upcasts) -> String;
}

impl ToParameter for library::Parameter {
    fn to_parameter(&self, library: &library::Library, upcasts: &Upcasts) -> String {
        if self.instance_parameter {
            "&self".into()
        } else {
            //TODO: Rework out (without inout) parameters as return type, with checking that it last
            //  Ex. gtk_range_get_range_rect, gtk_scale_get_layout_offsets
            let mut type_str: String;
            match upcasts.get_parameter_type_alias(&self.name) {
                Some(t) => type_str = format!("&{}", t),
                None => {
                    let rust_type = parameter_rust_type(library, self.typ, self.direction);
                    let type_name = rust_type.as_str();
                    let kind = TypeKind::of(library, self.typ);
                    type_str = match kind {
                        TypeKind::Unknown => format!("/*Unknown kind*/{}", type_name),
                        _ => type_name.into()
                    }
                }
            }
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
