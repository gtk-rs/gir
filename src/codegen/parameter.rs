use env::Env;
use library::{self, Nullable};
use analysis::type_kind::TypeKind;
use analysis::rust_type::parameter_rust_type;
use analysis::upcasts::Upcasts;
use traits::*;

pub trait ToParameter {
    fn to_parameter(&self, env: &Env, upcasts: &Upcasts) -> String;
}

impl ToParameter for library::Parameter {
    fn to_parameter(&self, env: &Env, upcasts: &Upcasts) -> String {
        if self.instance_parameter {
            "&self".into()
        } else {
            let type_str: String;
            match upcasts.get_parameter_type_alias(&self.name) {
                Some(t) => {
                    if self.nullable {
                        type_str = format!("Option<&{}>", t)
                    }
                    else {
                        type_str = format!("&{}", t)
                    }
                }
                None => {
                    let rust_type = parameter_rust_type(env, self.typ, self.direction,
                        Nullable(self.nullable));
                    let type_name = rust_type.as_str();
                    let kind = TypeKind::of(&env.library, self.typ);
                    type_str = match kind {
                        TypeKind::Unknown => format!("/*Unknown kind*/{}", type_name),
                        _ => type_name.into()
                    }
                }
            }
            format_parameter(&self.name, &type_str)
        }
    }
}

fn format_parameter(name: &str, type_str: &str) -> String {
    format!("{}: {}", name, type_str)
}
