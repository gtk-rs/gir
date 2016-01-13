use config::gobjects::GObject;
use env::Env;
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
    pub c_type: String,
    pub get_type: String,
    pub parents: Vec<general::StatusedTypeId>,
    pub implements: Vec<general::StatusedTypeId>,
    pub has_children: bool,
    pub functions: Vec<functions::Info>,
    pub specials: special_functions::Infos,
    pub has_constructors: bool,
    pub has_methods: bool,
    pub has_functions: bool,
    pub imports: Imports,
    pub version: Option<Version>,
    pub cfg_condition: Option<String>,
}

impl Info {
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
    imports.add("glib::translate::*".into(), None);
    imports.add("ffi".into(), None);

    let parents = parents::analyze_class(env, klass, &mut imports);
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

    if has_children {
        imports.add("glib::object::IsA".into(), None);
    }

    let mut functions =
        functions::analyze(env, &klass.functions, class_tid, &obj, &mut imports);
    let specials = special_functions::extract(&mut functions);
    // `copy` will duplicate an object while `clone` just adds a reference
    special_functions::unhide(&mut functions, &specials, special_functions::Type::Copy);
    special_functions::analyze_imports(&specials, &mut imports);

    let version = functions.iter().filter_map(|f| f.version).min();

    //don't `use` yourself
    imports.remove(&name);

    let mut info = Info {
        full_name: full_name,
        class_tid: class_tid,
        name: name,
        c_type: klass.c_type.clone(),
        get_type: klass.glib_get_type.clone(),
        parents: parents,
        implements: implements,
        has_children: has_children,
        functions: functions,
        specials: specials,
        imports: imports,
        version: version,
        cfg_condition: obj.cfg_condition.clone(),
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

pub fn interface(env: &Env, obj: &GObject) -> Option<Info> {
    let full_name = obj.name.clone();

    let iface_tid = match env.library.find_type(0, &full_name) {
        Some(tid) => tid,
        None => return None,
    };

    let type_ = env.type_(iface_tid);

    let name: String = split_namespace_name(&full_name).1.into();

    let iface: &library::Interface = match type_.maybe_ref() {
        Some(iface) => iface,
        None => return None,
    };

    let mut imports = Imports::new();
    imports.add("glib::translate::*".into(), None);
    imports.add("ffi".into(), None);
    imports.add("glib::object::IsA".into(), None);

    let parents = parents::analyze_interface(env, iface, &mut imports);

    let functions =
        functions::analyze(env, &iface.functions, iface_tid, &obj, &mut imports);

    let version = functions.iter().filter_map(|f| f.version).min();

    //don't `use` yourself
    imports.remove(&name);

    let mut info = Info {
        full_name: full_name,
        class_tid: iface_tid,
        name: name,
        c_type: iface.c_type.clone(),
        get_type: iface.glib_get_type.clone(),
        parents: parents,
        has_children: true,
        functions: functions,
        imports: imports,
        version: version,
        cfg_condition: obj.cfg_condition.clone(),
        .. Default::default()
    };

    info.has_methods = !info.methods().is_empty();
    Some(info)
}
