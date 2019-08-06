use super::{
    child_properties::ChildProperties, imports::Imports, info_base::InfoBase,
    signatures::Signatures, *,
};
use crate::{config::gobjects::GObject, env::Env, library, nameutil::*, traits::*};
use log::info;
use std::ops::Deref;

#[derive(Debug, Default)]
pub struct Info {
    pub base: InfoBase,
    pub c_type: String,
    pub c_class_type: Option<String>,
    pub rust_class_type: Option<String>,
    pub get_type: String,
    pub is_interface: bool,
    pub supertypes: Vec<general::StatusedTypeId>,
    pub final_type: bool,
    pub generate_trait: bool,
    pub trait_name: String,
    pub has_constructors: bool,
    pub has_methods: bool,
    pub has_functions: bool,
    pub signals: Vec<signals::Info>,
    pub notify_signals: Vec<signals::Info>,
    pub properties: Vec<properties::Property>,
    pub builder_properties: Vec<properties::Property>,
    pub child_properties: ChildProperties,
    pub signatures: Signatures,
}

impl Info {
    pub fn has_signals(&self) -> bool {
        self.signals.iter().any(|s| s.trampoline.is_ok())
            || self.notify_signals.iter().any(|s| s.trampoline.is_ok())
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
    imports.add("glib::translate::*");
    imports.add(env.main_sys_crate_name());
    if obj.generate_display_trait {
        imports.add("std::fmt");
    }

    let supertypes = supertypes::analyze(env, class_tid, &mut imports);

    let final_type = klass.final_type;
    let trait_name = obj
        .trait_name
        .as_ref()
        .cloned()
        .unwrap_or_else(|| format!("{}Ext", name));

    let mut signatures = Signatures::with_capacity(klass.functions.len());

    let mut functions = functions::analyze(
        env,
        &klass.functions,
        class_tid,
        !final_type,
        false,
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
    for t in &[
        special_functions::Type::Hash,
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
        !final_type,
        obj,
        &mut imports,
    );
    let (properties, notify_signals) = properties::analyze(
        env,
        &klass.properties,
        class_tid,
        !final_type,
        obj,
        &mut imports,
        &signatures,
        deps,
    );

    let builder_properties =
        class_builder::analyze(env, &klass.properties, class_tid, obj, &mut imports);

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
    let has_signals = signals.iter().any(|s| s.trampoline.is_ok())
        || notify_signals.iter().any(|s| s.trampoline.is_ok());

    // There's no point in generating a trait if there are no signals, methods, properties
    // and child properties: it would be empty
    //
    // There's also no point in generating a trait for final types: there are no possible subtypes
    let generate_trait = !final_type
        && (has_signals || has_methods || !properties.is_empty() || !child_properties.is_empty());

    if !builder_properties.is_empty() {
        imports.add("glib::object::Cast");
        imports.add("glib::StaticType");
        imports.add("glib::ToValue");
    }

    if generate_trait {
        imports.add("glib::object::IsA");
    }

    if obj.concurrency == library::Concurrency::SendUnique {
        imports.add("glib::ObjectExt");
    }

    let rust_class_type = Some(format!("{}Class", name));

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
        c_type: klass.c_type.clone(),
        c_class_type: klass.c_class_type.clone(),
        rust_class_type,
        get_type: klass.glib_get_type.clone(),
        is_interface: false,
        supertypes,
        final_type,
        generate_trait,
        trait_name,
        has_constructors,
        has_methods,
        has_functions,
        signals,
        notify_signals,
        properties,
        builder_properties,
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
    imports.add("glib::translate::*");
    imports.add(env.main_sys_crate_name());
    imports.add("glib::object::IsA");
    if obj.generate_display_trait {
        imports.add("std::fmt");
    }

    let supertypes = supertypes::analyze(env, iface_tid, &mut imports);

    let trait_name = obj
        .trait_name
        .as_ref()
        .cloned()
        .unwrap_or_else(|| format!("{}Ext", name));

    let mut signatures = Signatures::with_capacity(iface.functions.len());

    let functions = functions::analyze(
        env,
        &iface.functions,
        iface_tid,
        true,
        false,
        obj,
        &mut imports,
        Some(&mut signatures),
        Some(deps),
    );

    let signals = signals::analyze(env, &iface.signals, iface_tid, true, obj, &mut imports);
    let (properties, notify_signals) = properties::analyze(
        env,
        &iface.properties,
        iface_tid,
        true,
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
        imports.add("glib::ObjectExt");
    }

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
        c_type: iface.c_type.clone(),
        c_class_type: None,
        rust_class_type: None,
        get_type: iface.glib_get_type.clone(),
        is_interface: true,
        supertypes,
        final_type: false,
        generate_trait: true,
        trait_name,
        has_methods,
        has_functions,
        signals,
        notify_signals,
        properties,
        signatures,
        ..Default::default()
    };

    Some(info)
}
