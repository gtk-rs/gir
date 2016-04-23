use std::ops::Deref;

use config::gobjects::GObject;
use env::Env;
use library;
use nameutil::*;
use super::*;
use super::imports::Imports;
use super::info_base::InfoBase;
use traits::*;

#[derive(Default)]
pub struct Info<'e> {
    pub base: InfoBase<'e>,
    pub c_type: String,
    pub get_type: String,
    pub supertypes: Vec<general::StatusedTypeId<'e>>,
    pub has_children: bool,
    pub has_constructors: bool,
    pub has_methods: bool,
    pub has_functions: bool,
    pub signals: Vec<signals::Info<'e>>,
    pub trampolines: trampolines::Trampolines<'e>,
}

impl<'e> Info<'e> {
    pub fn has_signals(&self) -> bool {
        self.signals.iter().any(|s| s.trampoline_name.is_some())
    }
}

impl<'e> Deref for Info<'e> {
    type Target = InfoBase<'e>;

    fn deref(&self) -> &InfoBase<'e> {
        &self.base
    }
}

pub fn class<'e>(env: &'e Env, obj: &GObject) -> Option<Info<'e>> {
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
    imports.add("glib::translate::*", None);
    imports.add("ffi", None);

    let supertypes = supertypes::analyze(env, class_tid, &mut imports);

    let mut has_children = obj.force_trait;

    for child_tid in env.class_hierarchy.subtypes(class_tid) {
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
        imports.add("glib::object::IsA", None);
    }

    let mut trampolines = trampolines::Trampolines::new();

    let mut functions =
        functions::analyze(env, &klass.functions, class_tid, &obj, &mut imports);
    let specials = special_functions::extract(&mut functions);
    // `copy` will duplicate an object while `clone` just adds a reference
    special_functions::unhide(&mut functions, &specials, special_functions::Type::Copy);
    special_functions::analyze_imports(&specials, &mut imports);

    let signals = signals::analyze(env, &klass.signals, class_tid, has_children,
                                   &mut trampolines, &obj, &mut imports);

    let (version, deprecated_version) = info_base::versions(env, &obj, &functions, klass.version,
         klass.deprecated_version);

    //don't `use` yourself
    imports.remove(&name);

    let base = InfoBase {
        full_name: full_name,
        type_id: class_tid,
        name: name,
        functions: functions,
        specials: specials,
        imports: imports,
        version: version,
        deprecated_version: deprecated_version,
        cfg_condition: obj.cfg_condition.clone(),
    };

    // patch up trait methods in the symbol table
    if has_children {
        let mut symbols = env.symbols.borrow_mut();
        for func in base.methods() {
            if let Some(symbol) = symbols.by_c_name_mut(&func.glib_name) {
                symbol.make_trait_method();
            }
        }
    }

    let has_constructors = !base.constructors().is_empty();
    let has_methods = !base.methods().is_empty();
    let has_functions = !base.functions().is_empty();

    let info = Info {
        base: base,
        c_type: klass.c_type.clone(),
        get_type: klass.glib_get_type.clone(),
        supertypes: supertypes,
        has_children: has_children,
        has_constructors: has_constructors,
        has_methods: has_methods,
        has_functions: has_functions,
        signals: signals,
        trampolines: trampolines,
    };

    Some(info)
}

pub fn interface<'e>(env: &'e Env, obj: &GObject) -> Option<Info<'e>> {
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
    imports.add("glib::translate::*", None);
    imports.add("ffi", None);
    imports.add("glib::object::IsA", None);

    let supertypes = supertypes::analyze(env, iface_tid, &mut imports);

    let mut trampolines = trampolines::Trampolines::new();

    let functions =
        functions::analyze(env, &iface.functions, iface_tid, &obj, &mut imports);

    let signals = signals::analyze(env, &iface.signals, iface_tid, true,
                                   &mut trampolines, &obj, &mut imports);

    let (version, deprecated_version) = info_base::versions(env, &obj, &functions, iface.version,
         iface.deprecated_version);

    //don't `use` yourself
    imports.remove(&name);

    let base = InfoBase {
        full_name: full_name,
        type_id: iface_tid,
        name: name,
        functions: functions,
        specials: Default::default(),
        imports: imports,
        version: version,
        deprecated_version: deprecated_version,
        cfg_condition: obj.cfg_condition.clone(),
    };

    let has_methods = !base.methods().is_empty();

    let info = Info {
        base: base,
        c_type: iface.c_type.clone(),
        get_type: iface.glib_get_type.clone(),
        supertypes: supertypes,
        has_children: true,
        has_methods: has_methods,
        signals: signals,
        trampolines: trampolines,
        .. Default::default()
    };

    Some(info)
}
