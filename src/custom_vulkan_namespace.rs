use crate::library::*;

pub const VULKAN_NAMESPACE_NAME: &str = "Vulkan";

impl Library {
    pub fn tweak_vulkan_namespace(&mut self) {
        if let Some(ns_id) = self.find_namespace(VULKAN_NAMESPACE_NAME) {
            let ns = self.namespace_mut(ns_id);
            for typ in &mut ns.types {
                if let Some(Type::Record(rec)) = typ {
                    *typ = Some(Type::Basic(Basic::Typedef(format!(
                        "ash::vk::{}",
                        rec.name
                    ))));
                }
            }
        }
    }
}
