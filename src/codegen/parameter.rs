use crate::{
    analysis::{
        bounds::Bounds, function_parameters::CParameter, ref_mode::RefMode, rust_type::RustType,
    },
    env::Env,
    traits::*,
};

pub trait ToParameter {
    fn to_parameter(&self, env: &Env, bounds: &Bounds, r#async: bool) -> String;
}

impl ToParameter for CParameter {
    fn to_parameter(&self, env: &Env, bounds: &Bounds, r#async: bool) -> String {
        let ref_mode = if self.move_ {
            RefMode::None
        } else {
            self.ref_mode
        };
        if self.instance_parameter {
            format!("{ref_mode}self")
        } else {
            let type_str = if let Some(bound) = bounds.get_parameter_bound(&self.name) {
                bound.full_type_parameter_reference(ref_mode, self.nullable, r#async)
            } else {
                RustType::builder(env, self.typ)
                    .direction(self.direction)
                    .nullable(self.nullable)
                    .ref_mode(ref_mode)
                    .scope(self.scope)
                    .try_from_glib(&self.try_from_glib)
                    .try_build_param()
                    .into_string()
            };
            format!("{}: {}", self.name, type_str)
        }
    }
}
