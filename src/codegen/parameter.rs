use env::Env;
use analysis::bounds::{Bounds, BoundType};
use analysis::conversion_type::ConversionType;
use analysis::parameter::Parameter;
use analysis::ref_mode::RefMode;
use analysis::rust_type::parameter_rust_type;
use traits::*;

pub trait ToParameter {
    fn to_parameter(&self, env: &Env, bounds: &Bounds) -> String;
}

impl ToParameter for Parameter {
    fn to_parameter(&self, env: &Env, bounds: &Bounds) -> String {
        let mut_str = if self.ref_mode == RefMode::ByRefMut { "mut " } else { "" };
        if self.instance_parameter {
            format!("&{}self", mut_str)
        } else {
            let type_str: String;
            match bounds.get_parameter_alias_info(&self.name) {
                Some((t, bound_type)) => {
                    match bound_type {
                        BoundType::IsA => if *self.nullable {
                            type_str = format!("Option<&{}{}>", mut_str, t)
                        } else {
                            type_str = format!("&{}{}", mut_str, t)
                        },
                        BoundType::Into(_, Some(_)) => {
                            type_str = format!("{}", t)
                        }
                        BoundType::AsRef | BoundType::Into(_, None) => type_str = t.to_string(),
                    }
                }
                None => {
                    let rust_type = parameter_rust_type(env, self.typ, self.direction,
                                                        self.nullable, self.ref_mode);
                    let type_name = rust_type.into_string();
                    type_str = match ConversionType::of(&env.library, self.typ) {
                        ConversionType::Unknown => format!("/*Unknown conversion*/{}", type_name),
                        _ => type_name,
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
