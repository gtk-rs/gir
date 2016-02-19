use analysis;
use config::Config;
use config::gobjects::GStatus;
use library::*;
use std::cell::RefCell;

pub struct Env {
    pub library: Library,
    pub config: Config,
    pub namespaces: analysis::namespaces::Info,
    pub symbols: RefCell<analysis::symbols::Info>,
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
}
