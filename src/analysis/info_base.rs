use library;
use super::*;
use super::imports::Imports;
use version::Version;

#[derive(Default)]
pub struct InfoBase {
    pub full_name: String,
    pub type_id: library::TypeId,
    pub name: String,
    pub functions: Vec<functions::Info>,
    pub specials: special_functions::Infos,
    pub imports: Imports,
    pub version: Option<Version>,
    pub cfg_condition: Option<String>,
}

impl InfoBase {
    ///TODO: return iterator
    pub fn constructors(&self) -> Vec<&functions::Info> {
        self.functions.iter()
            .filter(|f| f.kind == library::FunctionKind::Constructor)
            .collect()
    }

    pub fn methods(&self) -> Vec<&functions::Info> {
        self.functions.iter()
            .filter(|f| f.kind == library::FunctionKind::Method)
            .collect()
    }

    pub fn functions(&self) -> Vec<&functions::Info> {
        self.functions.iter()
            .filter(|f| f.kind == library::FunctionKind::Function)
            .collect()
    }
}
