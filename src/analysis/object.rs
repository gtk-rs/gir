use env::Env;
use gobjects::GObject;
use library;
use nameutil::*;
use super::*;
use super::imports::Imports;
use traits::*;
use version::Version;

#[derive(Default)]
pub struct Info {
    pub full_name: String,
    pub class_tid: library::TypeId,
    pub name: String,
    pub parents: Vec<general::StatusedTypeId>,
    pub implements: Vec<general::StatusedTypeId>,
    pub has_children: bool,
    pub functions: Vec<functions::Info>,
    pub has_constructors: bool,
    pub has_methods: bool,
    pub has_functions: bool,
    pub imports: Imports,
    pub version: Option<Version>,
}

impl Info {
    //TODO: add test in tests/ for panic
    pub fn type_<'a>(&self, library: &'a library::Library) -> &'a library::Class {
        let type_ = library.type_(self.class_tid).maybe_ref()
            .unwrap_or_else(|| panic!("{} is not a class.", self.full_name));
        type_
    }

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

pub fn new(env: &Env, obj: &GObject) -> Info {
    let mut imports = Imports::new();
    let full_name = obj.name.clone();

    let class_tid = env.library.find_type_unwrapped(0, &full_name, "Class");

    let type_ = env.type_(class_tid);

    let name: String = split_namespace_name(&full_name).1.into();

    let klass = type_.to_ref();
    let parents = parents::analyze(env, klass, &mut imports);
    let implements = implements::analyze(env, klass, &mut imports);

    let mut has_children = false;

    for child_tid in &klass.children {
        let child_name = child_tid.full_name(&env.library);
        let status = env.config.objects.get(&child_name)
            .map(|o| o.status)
            .unwrap_or(Default::default());
        if status.normal() {
            has_children = true;
            break;
        }
    }

    let functions =
        functions::analyze(env, klass, class_tid, &obj.non_nullable_overrides, &mut imports);

    let version = functions.iter().filter_map(|f| f.version).min();

    //don't `use` yourself
    imports.remove(&name);

    let mut info = Info {
        full_name: full_name,
        class_tid: class_tid,
        name: name,
        parents: parents,
        implements: implements,
        has_children: has_children,
        functions: functions,
        imports: imports,
        version: version,
        .. Default::default()
    };

    let has_constructors = !info.constructors().is_empty();
    let has_methods = !info.methods().is_empty();
    let has_functions = !info.functions().is_empty();

    info.has_constructors = has_constructors;
    info.has_methods = has_methods;
    info.has_functions = has_functions;
    info
}
