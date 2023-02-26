use super::{imports::Imports, *};
use crate::{codegen::Visibility, config::gobjects::GStatus, library, version::Version};

#[derive(Debug, Default)]
pub struct InfoBase {
    pub full_name: String,
    pub type_id: library::TypeId,
    pub name: String,
    pub functions: Vec<functions::Info>,
    pub specials: special_functions::Infos,
    pub imports: Imports,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub concurrency: library::Concurrency,
    pub visibility: Visibility,
}

impl InfoBase {
    /// TODO: return iterator
    pub fn constructors(&self) -> Vec<&functions::Info> {
        self.functions
            .iter()
            .filter(|f| f.status.need_generate() && f.kind == library::FunctionKind::Constructor)
            .collect()
    }

    pub fn methods(&self) -> Vec<&functions::Info> {
        self.functions
            .iter()
            .filter(|f| f.status.need_generate() && f.kind == library::FunctionKind::Method)
            .collect()
    }

    pub fn functions(&self) -> Vec<&functions::Info> {
        self.functions
            .iter()
            .filter(|f| f.status.need_generate() && f.kind == library::FunctionKind::Function)
            .collect()
    }

    pub fn default_constructor(&self) -> Option<&functions::Info> {
        self.functions.iter().find(|f| {
            !f.hidden
                && f.status.need_generate()
                && f.kind == library::FunctionKind::Constructor
                && f.status == GStatus::Generate
                // For now we only look for new() with no params
                && f.name == "new"
                && f.parameters.rust_parameters.is_empty()
                // Cannot generate Default implementation for Option<>
                && f.ret.parameter.as_ref().map_or(false, |x| !*x.lib_par.nullable)
        })
    }
}
