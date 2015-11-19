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

pub fn new(env: &Env, obj: &GObject) -> Option<Info> {
    let full_name = obj.name.clone();

    let class_tid = match env.library.find_type(0, &full_name) {
        Some(tid) => tid,
        None => return None,
    };

    let type_ = env.type_(class_tid);

    let name: String = split_namespace_name(&full_name).1.into();

    let klass: &library::Class = match type_.maybe_ref() {
        Some(klass) => klass,
        None => return None,
    };

    let mut imports = Imports::new();
    imports.add("object::*".into(), None);
    imports.add("glib::translate::*".into(), None);
    imports.add("glib::types".into(), None);
    imports.add("ffi".into(), None);

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
        functions::analyze(env, &klass.functions, class_tid, &obj.non_nullable_overrides,
                           &obj.ignored_functions, &mut imports);

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
    Some(info)
}
