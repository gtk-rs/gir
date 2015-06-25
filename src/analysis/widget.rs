use env::Env;
use gobjects::*;
use library;
use nameutil::*;
use super::general;

pub struct Info {
    pub full_name: String,
    pub class_id: library::TypeId,
    pub name: String,
    pub parents: Vec<general::StatusedTypeId>,
}

impl Info {
    //TODO: add test in tests/ for panic
    pub fn type_<'a>(&self, library: &'a library::Library) -> &'a library::Class {
        let type_ = library.type_(self.class_id).as_class()
            .unwrap_or_else(|| panic!("{} is not a class.", self.full_name));
        type_
    }
}

pub fn new(env: &Env, obj: &GObject) -> Info {
    let full_name = obj.name.clone();

    let class_id = env.library.find_type_unwrapped(0, &full_name, "Class");

    let name = split_namespace_name(&full_name).1.into();

    let parents = general::analyze_parents(env, class_id);

    Info {
        full_name: full_name,
        class_id: class_id,
        name: name,
        parents: parents,
    }
}
