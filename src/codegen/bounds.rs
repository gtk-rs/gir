use crate::analysis::bounds::Bounds;

impl Bounds {
    pub fn to_generic_params_str(&self) -> String {
        self.to_generic_params_str_(false)
    }

    pub fn to_generic_params_str_async(&self) -> String {
        self.to_generic_params_str_(true)
    }

    fn to_generic_params_str_(&self, r#async: bool) -> String {
        let mut res = String::new();

        if self.lifetimes.is_empty() && self.used.iter().find_map(|b| b.alias).is_none() {
            return res;
        }

        res.push('<');
        let mut is_first = true;

        for lt in self.lifetimes.iter() {
            if is_first {
                is_first = false;
            } else {
                res.push_str(", ");
            }
            res.push('\'');
            res.push(*lt);
        }

        for bound in self.used.iter() {
            if let Some(type_param_def) = bound.type_parameter_definition(r#async) {
                if is_first {
                    is_first = false;
                } else {
                    res.push_str(", ");
                }
                res.push_str(&type_param_def);
            }
        }
        res.push('>');

        res
    }
}
