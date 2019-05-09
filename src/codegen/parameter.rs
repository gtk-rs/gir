use crate::env::Env;
use crate::analysis::bounds::{BoundType, Bounds};
use crate::analysis::conversion_type::ConversionType;
use crate::analysis::function_parameters::CParameter;
use crate::analysis::ref_mode::RefMode;
use crate::analysis::rust_type::parameter_rust_type;
use crate::traits::*;

pub trait ToParameter {
    fn to_parameter(&self, env: &Env, bounds: &Bounds) -> String;
}

impl ToParameter for CParameter {
    fn to_parameter(&self, env: &Env, bounds: &Bounds) -> String {
        let mut_str = if self.ref_mode == RefMode::ByRefMut {
            "mut "
        } else {
            ""
        };
        if self.instance_parameter {
            format!("&{}self", mut_str)
        } else {
            let type_str: String;
            match bounds.get_parameter_alias_info(&self.name) {
                Some((t, bound_type)) => {
                    match bound_type {
                        BoundType::NoWrapper => type_str = t.to_string(),
                        BoundType::IsA(_) if *self.nullable => {
                            type_str = format!("Option<&{}{}>", mut_str, t)
                        }
                        BoundType::IsA(_) => {
                            type_str = format!("&{}{}", mut_str, t)
                        }
                        BoundType::AsRef(_) => type_str = t.to_string(),
                    }
                }
                None => {
                    let rust_type = parameter_rust_type(
                        env,
                        self.typ,
                        self.direction,
                        self.nullable,
                        self.ref_mode,
                        self.scope,
                    );
                    let type_name = rust_type.into_string();
                    type_str = match ConversionType::of(env, self.typ) {
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
