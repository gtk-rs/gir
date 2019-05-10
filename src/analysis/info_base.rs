use super::{functions::Visibility, imports::Imports, *};
use crate::{config::gobjects::GObject, env::Env, library, version::Version};
use std::cmp;

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
}

impl InfoBase {
    ///TODO: return iterator
    pub fn constructors(&self) -> Vec<&functions::Info> {
        self.functions
            .iter()
            .filter(|f| f.kind == library::FunctionKind::Constructor)
            .collect()
    }

    pub fn methods(&self) -> Vec<&functions::Info> {
        self.functions
            .iter()
            .filter(|f| f.kind == library::FunctionKind::Method)
            .collect()
    }

    pub fn functions(&self) -> Vec<&functions::Info> {
        self.functions
            .iter()
            .filter(|f| f.kind == library::FunctionKind::Function)
            .collect()
    }
}

pub fn versions(
    env: &Env,
    obj: &GObject,
    functions: &[functions::Info],
    version: Option<Version>,
    deprecated_version: Option<Version>,
) -> (Option<Version>, Option<Version>) {
    let version = if obj.version.is_some() {
        obj.version
    } else {
        let fn_version = functions
            .iter()
            .filter(|f| f.visibility == Visibility::Public)
            .map(|f| f.version)
            .min()
            .unwrap_or(None);
        cmp::max(version, fn_version)
    };
    let version = env.config.filter_version(version);

    let fn_deprecated_max = functions
        .iter()
        .filter(|f| f.visibility == Visibility::Public)
        .map(|f| f.deprecated_version)
        .max()
        .unwrap_or(None);
    let fn_deprecated_min = functions
        .iter()
        .filter(|f| f.visibility == Visibility::Public)
        .map(|f| f.deprecated_version)
        .min()
        .unwrap_or(None);
    let deprecated_version =
        deprecated_version.or_else(|| fn_deprecated_min.and(fn_deprecated_max));

    (version, deprecated_version)
}
