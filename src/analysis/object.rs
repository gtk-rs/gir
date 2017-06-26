use std::ops::Deref;

use config::gobjects::GObject;
use env::Env;
use library;
use nameutil::*;
use super::*;
use super::child_properties::ChildProperties;
use super::imports::Imports;
use super::info_base::InfoBase;
use super::signatures::Signatures;
use traits::*;

#[derive(Default)]
pub struct Info {
    pub base: InfoBase,
    pub c_type: String,
    pub get_type: String,
    pub supertypes: Vec<general::StatusedTypeId>,
    pub generate_trait: bool,
    pub has_constructors: bool,
    pub has_methods: bool,
    pub has_functions: bool,
    pub signals: Vec<signals::Info>,
    pub trampolines: trampolines::Trampolines,
    pub properties: Vec<properties::Property>,
    pub child_properties: ChildProperties,
    pub signatures: Signatures,
}

impl Info {
    pub fn has_signals(&self) -> bool {
        self.signals.iter().any(|s| s.trampoline_name.is_ok())
    }
}

impl Deref for Info {
    type Target = InfoBase;

    fn deref(&self) -> &InfoBase {
        &self.base
    }
}

pub fn has_known_subtypes(env: &Env, class_tid: library::TypeId) -> bool {
    for child_tid in env.class_hierarchy.subtypes(class_tid) {
        let child_name = child_tid.full_name(&env.library);
        let status = env.config
            .objects
            .get(&child_name)
            .map(|o| o.status)
            .unwrap_or_default();
        if status.normal() {
            return true;
        }
    }

    false
}

pub fn class(env: &Env, obj: &GObject, deps: &[library::TypeId]) -> Option<Info> {
    info!("Analyzing class {}", obj.name);
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

    let mut generate_trait = obj.generate_trait;

    // Sanity check the user's configuration. It's unlikely that not generating
    // a trait is wanted if there are subtypes in this very crate
    if !generate_trait && has_known_subtypes(env, class_tid) {
        error!(
            "Not generating trait for {} although subtypes exist",
            full_name
        );
    }

    let mut trampolines = trampolines::Trampolines::with_capacity(klass.signals.len());
    let mut signatures = Signatures::with_capacity(klass.functions.len());

    let mut functions = functions::analyze(
        env,
        &klass.functions,
        class_tid,
        obj,
        &mut imports,
        Some(&mut signatures),
        Some(deps),
    );
    let specials = special_functions::extract(&mut functions);
    // `copy` will duplicate an object while `clone` just adds a reference
    special_functions::unhide(&mut functions, &specials, special_functions::Type::Copy);
    special_functions::analyze_imports(&specials, &mut imports);

    let signals = signals::analyze(
        env,
        &klass.signals,
        class_tid,
        generate_trait,
        &mut trampolines,
        obj,
        &mut imports,
    );
    let properties = properties::analyze(
        env,
        &klass.properties,
        class_tid,
        obj,
        &mut imports,
        &signatures,
        deps,
    );

    let (version, deprecated_version) = info_base::versions(
        env,
        obj,
        &functions,
        klass.version,
        klass.deprecated_version,
    );

    let child_properties =
        child_properties::analyze(env, obj.child_properties.as_ref(), class_tid, &mut imports);

    let has_methods = functions
        .iter()
        .any(|f| f.kind == library::FunctionKind::Method);
    let has_signals = signals.iter().any(|s| s.trampoline_name.is_ok());

    // There's no point in generating a trait if there are no signals, methods, properties
    // and child properties: it would be empty
    if generate_trait && !has_signals && !has_methods && properties.is_empty() &&
        child_properties.is_empty()
    {
        generate_trait = false;
    }

    if generate_trait && (!properties.is_empty() || has_signals) {
        imports.add("glib", None);
    }
    if generate_trait &&
        (has_methods || !properties.is_empty() || !child_properties.is_empty() || has_signals)
    {
        imports.add("glib::object::IsA", None);
    }

    //don't `use` yourself
    imports.remove(&name);

    imports.clean_glib(env);

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
        concurrency: obj.concurrency,
    };

    // patch up trait methods in the symbol table
    if generate_trait {
        let mut symbols = env.symbols.borrow_mut();
        for func in base.methods() {
            if let Some(symbol) = symbols.by_c_name_mut(&func.glib_name) {
                symbol.make_trait_method();
            }
        }
    }

    let has_constructors = !base.constructors().is_empty();
    let has_functions = !base.functions().is_empty();

    let info = Info {
        base: base,
        c_type: klass.c_type.clone(),
        get_type: klass.glib_get_type.clone(),
        supertypes: supertypes,
        generate_trait: generate_trait,
        has_constructors: has_constructors,
        has_methods: has_methods,
        has_functions: has_functions,
        signals: signals,
        trampolines: trampolines,
        properties: properties,
        child_properties: child_properties,
        signatures: signatures,
    };

    Some(info)
}

pub fn interface(env: &Env, obj: &GObject, deps: &[library::TypeId]) -> Option<Info> {
    info!("Analyzing interface {}", obj.name);
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

    let mut trampolines = trampolines::Trampolines::with_capacity(iface.signals.len());
    let mut signatures = Signatures::with_capacity(iface.functions.len());

    let functions = functions::analyze(
        env,
        &iface.functions,
        iface_tid,
        obj,
        &mut imports,
        Some(&mut signatures),
        Some(deps),
    );

    let signals = signals::analyze(
        env,
        &iface.signals,
        iface_tid,
        true,
        &mut trampolines,
        obj,
        &mut imports,
    );
    let properties = properties::analyze(
        env,
        &iface.properties,
        iface_tid,
        obj,
        &mut imports,
        &signatures,
        deps,
    );

    let (version, deprecated_version) = info_base::versions(
        env,
        obj,
        &functions,
        iface.version,
        iface.deprecated_version,
    );

    if !properties.is_empty() {
        imports.add("glib", None);
    }

    //don't `use` yourself
    imports.remove(&name);

    imports.clean_glib(env);

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
        concurrency: obj.concurrency,
    };

    let has_methods = !base.methods().is_empty();
    let has_functions = !base.functions().is_empty();

    let info = Info {
        base: base,
        c_type: iface.c_type.clone(),
        get_type: iface.glib_get_type.clone(),
        supertypes: supertypes,
        generate_trait: true,
        has_methods: has_methods,
        has_functions: has_functions,
        signals: signals,
        trampolines: trampolines,
        properties: properties,
        signatures: signatures,
        ..Default::default()
    };

    Some(info)
}
