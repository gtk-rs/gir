use env::Env;
use analysis::conversion_type::ConversionType;
use analysis::parameter::Parameter;
use analysis::ref_mode::RefMode;
use analysis::rust_type::parameter_rust_type;
use analysis::upcasts::Upcasts;
use traits::*;

pub trait ToParameter {
    fn to_parameter(&self, env: &Env, upcasts: &Upcasts) -> String;
}

impl ToParameter for Parameter {
    fn to_parameter(&self, env: &Env, upcasts: &Upcasts) -> String {
        let mut_str = if self.ref_mode == RefMode::ByRefMut { "mut " } else { "" };
        if self.instance_parameter {
            format!("&{}self", mut_str)
        } else {
            let type_str: String;
            match upcasts.get_parameter_type_alias(&self.name) {
                Some(t) => {
                    if *self.nullable {
                        type_str = format!("Option<&{}{}>", mut_str, t)
                    }
                    else {
                        type_str = format!("&{}{}", mut_str, t)
                    }
                }
                None => {
                    let rust_type = parameter_rust_type(env, self.typ, self.direction,
                                                        self.nullable, self.ref_mode);
                    let type_name = rust_type.as_str();
                    type_str = match ConversionType::of(&env.library, self.typ) {
                        ConversionType::Unknown => format!("/*Unknown conversion*/{}", type_name),
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
