use env::Env;
use gobjects::GObject;
use super::object;

pub fn new(env: &Env, obj: &GObject) -> object::Info {
    let info = object::new(env, obj);

    info
}
