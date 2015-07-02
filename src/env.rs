use config::Config;
use library::*;

pub struct Env {
    pub library: Library,
    pub config: Config,
}

impl Env {
    #[inline]
    pub fn type_(&self, tid: TypeId) -> &Type {
        self.library.type_(tid)
    }
}
