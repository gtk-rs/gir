use crate::{
    analysis::{
        bounds::{BoundType, Bounds},
        conversion_type::ConversionType,
        function_parameters::CParameter,
        ref_mode::RefMode,
        rust_type::RustType,
    },
    env::Env,
    traits::*,
};

pub trait ToParameter {
    fn to_parameter(&self, env: &Env, bounds: &Bounds) -> String;
}

impl ToParameter for CParameter {
    fn to_parameter(&self, env: &Env, bounds: &Bounds) -> String {
        let ref_str = match self.ref_mode {
            RefMode::ByRefMut => "&mut ",
            RefMode::None => "",
            _ => "&",
        };
        if self.instance_parameter {
            format!("{}self", ref_str)
        } else {
            let type_str: String;
            match bounds.get_parameter_alias_info(&self.name) {
                Some((t, bound_type)) => match bound_type {
                    BoundType::NoWrapper => type_str = t.to_string(),
                    BoundType::IsA(_) if *self.nullable => {
                        type_str = format!("Option<{}{}>", ref_str, t)
                    }
                    BoundType::IsA(_) => type_str = format!("{}{}", ref_str, t),
                    BoundType::AsRef(_) => type_str = t.to_string(),
                },
                None => {
                    let type_name = RustType::builder(env, self.typ)
                        .direction(self.direction)
                        .nullable(self.nullable)
                        .ref_mode(self.ref_mode)
                        .scope(self.scope)
                        .try_from_glib(&self.try_from_glib)
                        .try_build_param()
                        .into_string();
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
