//! Adds `glib::Priority` as custom type
//! and attempts replace priority parameters with it in async functions

use crate::{
    analysis::conversion_type::ConversionType, config::WorkMode, library::*,
    visitors::FunctionsMutVisitor,
};

impl Library {
    pub fn add_glib_priority(&mut self, work_mode: WorkMode) {
        if work_mode != WorkMode::Normal {
            return;
        }

        let tid_int = self.find_type(0, "*.gint").expect("No basic type *.gint");
        let glib_ns_id = self
            .find_namespace("GLib")
            .expect("Missing `GLib` namespace in add_glib_priority!");
        let tid_priority = self.add_type(
            glib_ns_id,
            "Priority",
            Type::Custom(Custom {
                name: "Priority".to_string(),
                conversion_type: ConversionType::Scalar,
            }),
        );

        let mut replacer = ReplaceToPriority {
            tid_priority,
            tid_int,
        };
        self.namespace_mut(MAIN_NAMESPACE)
            .visit_functions_mut(&mut replacer);
    }
}

struct ReplaceToPriority {
    pub tid_priority: TypeId,
    pub tid_int: TypeId,
}

impl FunctionsMutVisitor for ReplaceToPriority {
    fn visit_function_mut(&mut self, func: &mut Function) -> bool {
        if !func.name.ends_with("_async") {
            return true;
        }
        for par in &mut func.parameters {
            if par.typ == self.tid_int && par.name.ends_with("priority") {
                par.typ = self.tid_priority;
            }
        }
        true
    }
}
