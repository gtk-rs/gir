use std::cell::RefCell;

use analysis;
use config::Config;
use config::gobjects::GStatus;
use library::*;
use version::Version;

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
        self.config.objects.get(name).map(|o| o.status)
            .unwrap_or(Default::default())
    }
    pub fn type_status_sys(&self, name: &str) -> GStatus {
        self.config.objects.get(name).map(|o| o.status)
            .unwrap_or(GStatus::Generate)
    }

    pub fn is_totally_deprecated(&self, deprecated_version: Option<Version>) -> bool {
        match deprecated_version {
            Some(version) if version <= self.config.min_cfg_version =>
                self.config.deprecate_by_min_version,
            _ => false,
        }
    }
}
