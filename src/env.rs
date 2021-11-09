use crate::{
    analysis::{self, namespaces::NsId},
    config::{gobjects::GStatus, Config},
    library::*,
    version::Version,
};
use std::cell::RefCell;
use std::ops::Index;

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

    pub fn is_totally_deprecated(
        &self,
        ns_id: Option<NsId>,
        deprecated_version: Option<Version>,
    ) -> bool {
        let to_compare_with = ns_id
            .and_then(|ns| {
                let namespace = self.namespaces.index(ns);
                self.config
                    .find_ext_library(namespace)
                    .and_then(|lib| lib.min_version)
            })
            .unwrap_or(self.config.min_cfg_version);

        match deprecated_version {
            Some(version) if version <= to_compare_with => self.config.deprecate_by_min_version,
            _ => false,
        }
    }

    pub fn is_too_low_version(&self, ns_id: Option<NsId>, version: Option<Version>) -> bool {
        let to_compare_with = ns_id
            .and_then(|ns| {
                let namespace = self.namespaces.index(ns);
                self.config
                    .find_ext_library(namespace)
                    .and_then(|lib| lib.min_version)
            })
            .unwrap_or(self.config.min_cfg_version);

        match version {
            Some(version) => version <= to_compare_with,
            _ => false,
        }
    }

    pub fn main_sys_crate_name(&self) -> &str {
        &self.namespaces[MAIN_NAMESPACE].sys_crate_name
    }
}
