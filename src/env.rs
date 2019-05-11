use crate::{
    analysis,
    config::{gobjects::GStatus, Config},
    library::*,
    version::Version,
};
use std::cell::RefCell;

#[derive(Debug)]
pub struct Env {
    pub library: Library,
    pub config: Config,
    pub namespaces: analysis::namespaces::Info,
    pub symbols: RefCell<analysis::symbols::Info>,
    pub class_hierarchy: analysis::class_hierarchy::Info,
    pub analysis: analysis::Analysis,
}

impl Env {
    #[inline]
    pub fn type_(&self, tid: TypeId) -> &Type {
        self.library.type_(tid)
    }
    pub fn type_status(&self, name: &str) -> GStatus {
        self.config
            .objects
            .get(name)
            .map(|o| o.status)
            .unwrap_or_default()
    }
    pub fn type_status_sys(&self, name: &str) -> GStatus {
        self.config
            .objects
            .get(name)
            .map(|o| o.status)
            .unwrap_or(GStatus::Generate)
    }

    pub fn is_totally_deprecated(&self, deprecated_version: Option<Version>) -> bool {
        match deprecated_version {
            Some(version) if version <= self.config.min_cfg_version => {
                self.config.deprecate_by_min_version
            }
            _ => false,
        }
    }

    pub fn is_too_low_version(&self, version: Option<Version>) -> bool {
        match version {
            Some(version) => version <= self.config.min_cfg_version,
            _ => false,
        }
    }

    pub fn main_sys_crate_name(&self) -> &str {
        &self.namespaces[MAIN_NAMESPACE].sys_crate_name
    }
}
