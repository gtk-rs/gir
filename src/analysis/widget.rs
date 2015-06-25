use env::Env;
use gobjects::*;
use library;
use nameutil::*;

pub struct Info {
    pub full_name: String,
    pub class_id: library::TypeId,
    pub type_name: String,
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

    let class_id = env.library.find_type(0, &full_name)
        .unwrap_or_else(|| panic!("Class {} not found.", full_name));

    let type_name = split_namespace_name(&full_name).1.into();

    Info {
        full_name: full_name,
        class_id: class_id,
        type_name: type_name,
    }
}
