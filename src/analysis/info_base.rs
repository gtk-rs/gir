use super::{imports::Imports, *};
use crate::{codegen::Visibility, library, version::Version};

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
}
