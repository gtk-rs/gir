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

#[derive(Debug, Default)]
pub struct Info {
    pub base: InfoBase,
    pub is_interface: bool,
    pub c_type: String,
    pub class_type: Option<String>,
    pub c_class_type: Option<String>,
    pub rust_class_type: Option<String>,
    pub get_type: String,
    pub supertypes: Vec<general::StatusedTypeId>,
    pub generate_trait: bool,
    pub trait_name: String,
    pub subclass_impl_trait_name: String,
    pub subclass_base_trait_name: String,
    pub has_constructors: bool,
    pub has_methods: bool,
    pub has_functions: bool,
    pub signals: Vec<signals::Info>,
    pub notify_signals: Vec<signals::Info>,
    pub virtual_methods: Vec<virtual_methods::Info>,
    pub trampolines: trampolines::Trampolines,
    pub properties: Vec<properties::Property>,
    pub child_properties: ChildProperties,
    pub signatures: Signatures,
}

impl Info {
    pub fn has_signals(&self) -> bool {
        self.signals.iter().any(|s| s.trampoline_name.is_ok())
            || self.notify_signals
                .iter()
                .any(|s| s.trampoline_name.is_ok())
    }

    pub fn has_action_signals(&self) -> bool {
        self.signals.iter().any(|s| s.action_emit_name.is_some())
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

    let mut imports = Imports::with_defined(&env.library, &name);
    imports.add("glib::translate::*", None);
    imports.add("ffi", None);
    if obj.generate_display_trait {
        imports.add("std::fmt", None);
    }

    let supertypes = supertypes::analyze(env, class_tid, &mut imports);

    let mut generate_trait = obj.generate_trait;
    let trait_name = obj.trait_name
        .as_ref()
        .cloned()
        .unwrap_or_else(|| format!("{}Ext", name));

    let subclass_impl_trait_name = obj.subclass_impl_trait_name
        .as_ref()
        .cloned()
        .unwrap_or_else(|| format!("{}Impl", name));

    let subclass_base_trait_name = obj.subclass_base_trait_name
        .as_ref()
        .cloned()
        .unwrap_or_else(|| format!("{}Base", name));

    // Sanity check the user's configuration. It's unlikely that not generating
    // a trait is wanted if there are subtypes in this very crate
    if !generate_trait && has_known_subtypes(env, class_tid) {
        error!(
            "Not generating trait for {} although subtypes exist",
            full_name
        );
    }

    let mut trampolines =
        trampolines::Trampolines::with_capacity(klass.signals.len() + klass.properties.len());
    let mut signatures = Signatures::with_capacity(klass.functions.len());

    let mut functions = functions::analyze(
        env,
        &klass.functions,
        class_tid,
        generate_trait,
        obj,
        &mut imports,
        Some(&mut signatures),
        Some(deps),
    );
    let mut specials = special_functions::extract(&mut functions);
    // `copy` will duplicate an object while `clone` just adds a reference
    special_functions::unhide(&mut functions, &specials, special_functions::Type::Copy);
    // these are all automatically derived on objects and compare by pointer. If such functions
    // exist they will provide additional functionality
    for t in &[special_functions::Type::Hash,
              special_functions::Type::Equal,
              special_functions::Type::Compare,
              special_functions::Type::ToString,
    ] {
        special_functions::unhide(&mut functions, &specials, *t);
        specials.remove(t);
    }
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

    let virtual_methods = virtual_methods::analyze(
        env,
        &klass.virtual_methods,
        class_tid,
        generate_trait,
        obj,
        &mut imports,
        Some(&mut signatures),
        Some(deps),
    );

    let (properties, notify_signals) = properties::analyze(
        env,
        &klass.properties,
        class_tid,
        generate_trait,
        &mut trampolines,
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
    let has_signals = signals.iter().any(|s| s.trampoline_name.is_ok())
        || notify_signals.iter().any(|s| s.trampoline_name.is_ok());

    // There's no point in generating a trait if there are no signals, methods, properties
    // and child properties: it would be empty
    if generate_trait && !has_signals && !has_methods && properties.is_empty()
        && child_properties.is_empty()
    {
        generate_trait = false;
    }

    if generate_trait
        && (has_methods || !properties.is_empty() || !child_properties.is_empty() || has_signals)
    {
        imports.add("glib::object::IsA", None);
    }

    if obj.concurrency == library::Concurrency::SendUnique {
        imports.add("glib::ObjectExt", None);
    }

    let rust_class_type = if obj.subclassing {
        let class_name = format!("{}Class", name);
        imports.add(&class_name, None);
        Some(class_name)
    } else {
        None
    };

    let base = InfoBase {
        full_name,
        type_id: class_tid,
        name,
        functions,
        specials,
        imports,
        version,
        deprecated_version,
        cfg_condition: obj.cfg_condition.clone(),
        concurrency: obj.concurrency,
    };

    // patch up trait methods in the symbol table
    if generate_trait {
        let mut symbols = env.symbols.borrow_mut();
        for func in base.methods() {
            if let Some(symbol) = symbols.by_c_name_mut(&func.glib_name) {
                symbol.make_trait_method(&trait_name);
            }
        }
    }

    let has_constructors = !base.constructors().is_empty();
    let has_functions = !base.functions().is_empty();

    let info = Info {
        base,
        is_interface: false,
        c_type: klass.c_type.clone(),
        class_type: klass.type_struct.clone(),
        c_class_type: klass.c_class_type.clone(),
        rust_class_type,
        get_type: klass.glib_get_type.clone(),
        supertypes,
        generate_trait,
        trait_name,
        subclass_impl_trait_name,
        subclass_base_trait_name,
        has_constructors,
        has_methods,
        has_functions,
        signals,
        notify_signals,
        virtual_methods,
        trampolines,
        properties,
        child_properties,
        signatures,
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

    let mut imports = Imports::with_defined(&env.library, &name);
    imports.add("glib::translate::*", None);
    imports.add("ffi", None);
    imports.add("glib::object::IsA", None);
    if obj.generate_display_trait {
        imports.add("std::fmt", None);
    }

    let supertypes = supertypes::analyze(env, iface_tid, &mut imports);

    let trait_name = obj.trait_name
        .as_ref()
        .cloned()
        .unwrap_or_else(|| format!("{}Ext", name));

    let mut trampolines = trampolines::Trampolines::with_capacity(iface.signals.len());
    let mut signatures = Signatures::with_capacity(iface.functions.len());

    let functions = functions::analyze(
        env,
        &iface.functions,
        iface_tid,
        true,
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

    let virtual_methods = virtual_methods::analyze(
        env,
        &iface.virtual_methods,
        iface_tid,
        true,
        obj,
        &mut imports,
        Some(&mut signatures),
        Some(deps)
    );

    let (properties, notify_signals) = properties::analyze(
        env,
        &iface.properties,
        iface_tid,
        true,
        &mut trampolines,
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

    if obj.concurrency == library::Concurrency::SendUnique {
        imports.add("glib::ObjectExt", None);
    }

    let subclass_impl_trait_name = obj.subclass_impl_trait_name
        .as_ref()
        .cloned()
        .unwrap_or_else(|| format!("{}Impl", name));

    let base = InfoBase {
        full_name,
        type_id: iface_tid,
        name,
        functions,
        specials: Default::default(),
        imports,
        version,
        deprecated_version,
        cfg_condition: obj.cfg_condition.clone(),
        concurrency: obj.concurrency,
    };

    let has_methods = !base.methods().is_empty();
    let has_functions = !base.functions().is_empty();

    let info = Info {
        base,
        is_interface: true,
        c_type: iface.c_type.clone(),
        class_type: iface.type_struct.clone(),
        c_class_type: iface.c_class_type.clone(),
        rust_class_type: None,
        get_type: iface.glib_get_type.clone(),
        supertypes,
        generate_trait: true,
        trait_name,
        has_methods,
        has_functions,
        signals,
        notify_signals,
        virtual_methods,
        trampolines,
        properties,
        signatures,
        subclass_impl_trait_name,
        ..Default::default()
    };

    Some(info)
}
