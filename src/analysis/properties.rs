use crate::{
    analysis::{
        bounds::{Bounds, PropertyBound},
        imports::Imports,
        ref_mode::RefMode,
        rust_type::*,
        signals,
        signatures::{Signature, Signatures},
        trampolines,
    },
    config::{self, GObject, PropertyGenerateFlags},
    env::Env,
    library, nameutil,
    traits::*,
    version::Version,
};
use log::warn;

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub var_name: String,
    pub typ: library::TypeId,
    pub is_get: bool,
    pub func_name: String,
    pub nullable: library::Nullable,
    pub get_out_ref_mode: RefMode,
    pub set_in_ref_mode: RefMode,
    pub bounds: Bounds,
    pub set_bound: Option<PropertyBound>,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
}

pub fn analyze(
    env: &Env,
    props: &[library::Property],
    type_tid: library::TypeId,
    generate_trait: bool,
    obj: &GObject,
    imports: &mut Imports,
    signatures: &Signatures,
    deps: &[library::TypeId],
) -> (Vec<Property>, Vec<signals::Info>) {
    let mut properties = Vec::new();
    let mut notify_signals = Vec::new();

    for prop in props {
        let configured_properties = obj.properties.matched(&prop.name);
        if configured_properties.iter().any(|f| f.ignore) {
            continue;
        }

        if env.is_totally_deprecated(prop.deprecated_version) {
            continue;
        }

        let (getter, setter, notify_signal) = analyze_property(
            env,
            prop,
            type_tid,
            &configured_properties,
            generate_trait,
            obj,
            imports,
            signatures,
            deps,
        );

        if let Some(notify_signal) = notify_signal {
            notify_signals.push(notify_signal);
        }

        if let Some(prop) = getter {
            properties.push(prop);
        }
        if let Some(prop) = setter {
            properties.push(prop);
        }
    }

    (properties, notify_signals)
}

fn analyze_property(
    env: &Env,
    prop: &library::Property,
    type_tid: library::TypeId,
    configured_properties: &[&config::properties::Property],
    generate_trait: bool,
    obj: &GObject,
    imports: &mut Imports,
    signatures: &Signatures,
    deps: &[library::TypeId],
) -> (Option<Property>, Option<Property>, Option<signals::Info>) {
    let type_name = type_tid.full_name(&env.library);
    let name = prop.name.clone();

    let prop_version = configured_properties
        .iter()
        .filter_map(|f| f.version)
        .min()
        .or(prop.version)
        .or_else(|| Some(env.config.min_cfg_version));
    let generate = configured_properties
        .iter()
        .filter_map(|f| f.generate)
        .next();
    let generate_set = generate.is_some();
    let generate = generate.unwrap_or_else(PropertyGenerateFlags::all);

    imports.set_defaults(prop_version, &None);

    let type_string = rust_type(env, prop.typ);
    let name_for_func = nameutil::signal_to_snake(&name);
    let var_name = nameutil::mangle_keywords(&*name_for_func).into_owned();
    let get_func_name = format!("get_property_{}", name_for_func);
    let set_func_name = format!("set_property_{}", name_for_func);
    let check_get_func_name = format!("get_{}", name_for_func);
    let check_set_func_name = format!("set_{}", name_for_func);

    let mut readable = prop.readable;
    let mut writable = if prop.construct_only {
        false
    } else {
        prop.writable
    };
    let mut notifiable = !prop.construct_only;
    if generate_set && generate.contains(PropertyGenerateFlags::GET) && !readable {
        warn!(
            "Attempt to generate getter for notreadable property \"{}.{}\"",
            type_name, name
        );
    }
    if generate_set && generate.contains(PropertyGenerateFlags::SET) && !writable {
        warn!(
            "Attempt to generate setter for nonwritable property \"{}.{}\"",
            type_name, name
        );
    }
    readable &= generate.contains(PropertyGenerateFlags::GET);
    writable &= generate.contains(PropertyGenerateFlags::SET);
    if generate_set {
        notifiable = generate.contains(PropertyGenerateFlags::NOTIFY);
    }

    if readable {
        let (has, version) = Signature::has_for_property(
            env,
            &check_get_func_name,
            true,
            prop.typ,
            signatures,
            deps,
        );
        if has && (env.is_totally_deprecated(version) || version <= prop_version) {
            readable = false;
        }
    }
    if writable {
        let (has, version) = Signature::has_for_property(
            env,
            &check_set_func_name,
            false,
            prop.typ,
            signatures,
            deps,
        );
        if has && (env.is_totally_deprecated(version) || version <= prop_version) {
            writable = false;
        }
    }

    let (get_out_ref_mode, set_in_ref_mode, nullable) = get_property_ref_modes(env, prop);

    let getter = if readable {
        if let Ok(ref s) = used_rust_type(env, prop.typ, false) {
            imports.add_used_type(s);
        }
        if type_string.is_ok() {
            imports.add("gobject_sys");
            imports.add("glib::Value");
            imports.add("glib::StaticType");
        }

        Some(Property {
            name: name.clone(),
            var_name: String::new(),
            typ: prop.typ,
            is_get: true,
            func_name: get_func_name,
            nullable,
            get_out_ref_mode,
            set_in_ref_mode,
            set_bound: None,
            bounds: Bounds::default(),
            version: prop_version,
            deprecated_version: prop.deprecated_version,
        })
    } else {
        None
    };

    let setter = if writable {
        if let Ok(ref s) = used_rust_type(env, prop.typ, true) {
            imports.add_used_type(s);
        }
        let set_bound = PropertyBound::get(env, prop.typ);
        if type_string.is_ok() {
            imports.add("gobject_sys");
            imports.add("glib::Value");
            if set_bound.is_some() {
                imports.add("glib::object::IsA");
                imports.add("glib::value::SetValueOptional");
                if !*nullable {
                    //TODO: support nonnulable setter if found any
                    warn!(
                        "Non nulable setter for property generated as nullable \"{}.{}\"",
                        type_name, name
                    );
                }
            }
        }

        Some(Property {
            name: name.clone(),
            var_name,
            typ: prop.typ,
            is_get: false,
            func_name: set_func_name,
            nullable,
            get_out_ref_mode,
            set_in_ref_mode,
            set_bound,
            bounds: Bounds::default(),
            version: prop_version,
            deprecated_version: prop.deprecated_version,
        })
    } else {
        None
    };

    if !generate_trait && (writable || readable || notifiable) {
        //To resolve a conflict with OSTree::ObjectType
        imports.add("glib::object::ObjectType as ObjectType_");
    }

    let notify_signal = if notifiable {
        let mut used_types: Vec<String> = Vec::with_capacity(4);
        let trampoline = trampolines::analyze(
            env,
            &library::Signal {
                name: format!("notify::{}", name),
                parameters: Vec::new(),
                ret: library::Parameter {
                    name: "".into(),
                    typ: env
                        .library
                        .find_type(library::INTERNAL_NAMESPACE, "none")
                        .unwrap(),
                    c_type: "none".into(),
                    instance_parameter: false,
                    direction: library::ParameterDirection::Return,
                    transfer: library::Transfer::None,
                    caller_allocates: false,
                    nullable: library::Nullable(false),
                    allow_none: false,
                    array_length: None,
                    is_error: false,
                    doc: None,
                    scope: library::ParameterScope::None,
                    closure: None,
                    destroy: None,
                },
                is_action: false,
                version: prop_version,
                deprecated_version: prop.deprecated_version,
                doc: None,
                doc_deprecated: None,
            },
            type_tid,
            generate_trait,
            &[],
            obj,
            &mut used_types,
            prop_version,
        );

        if trampoline.is_ok() {
            imports.add_used_types(&used_types);
            if generate_trait {
                imports.add("glib::object::Cast");
            }
            imports.add("glib::signal::connect_raw");
            imports.add("glib::signal::SignalHandlerId");
            imports.add("std::mem::transmute");
            imports.add("std::boxed::Box as Box_");
            imports.add("glib_sys");

            Some(signals::Info {
                connect_name: format!("connect_property_{}_notify", name_for_func),
                signal_name: format!("notify::{}", name),
                trampoline,
                action_emit_name: None,
                version: prop_version,
                deprecated_version: prop.deprecated_version,
                doc_hidden: false,
            })
        } else {
            None
        }
    } else {
        None
    };

    imports.reset_defaults();

    (getter, setter, notify_signal)
}

pub fn get_property_ref_modes(
    env: &Env,
    prop: &library::Property,
) -> (RefMode, RefMode, library::Nullable) {
    let get_out_ref_mode = RefMode::of(env, prop.typ, library::ParameterDirection::Return);
    let mut set_in_ref_mode = RefMode::of(env, prop.typ, library::ParameterDirection::In);
    if set_in_ref_mode == RefMode::ByRefMut {
        set_in_ref_mode = RefMode::ByRef;
    }
    let nullable = library::Nullable(set_in_ref_mode.is_ref());
    (get_out_ref_mode, set_in_ref_mode, nullable)
}
