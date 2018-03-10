use analysis::bounds::Bound;
use analysis::imports::Imports;
use analysis::ref_mode::RefMode;
use analysis::rust_type::*;
use analysis::signatures::{Signature, Signatures};
use analysis::signals;
use analysis::trampolines;
use config;
use config::gobjects::GObject;
use env::Env;
use library;
use nameutil;
use traits::*;
use version::Version;

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
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub bound: Option<Bound>,
}

pub fn analyze(
    env: &Env,
    props: &[library::Property],
    type_tid: library::TypeId,
    generate_trait: bool,
    trampolines: &mut trampolines::Trampolines,
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
            trampolines,
            obj,
            imports,
            signatures,
            deps,
        );

        if let Some(notify_signal) = notify_signal {
            notify_signals.push(notify_signal);
        }

        if getter.is_none() && setter.is_none() {
            continue;
        }

        let type_string = rust_type(env, prop.typ);
        let used_type_string = used_rust_type(env, prop.typ);
        if let Some(prop) = getter {
            if let Ok(ref s) = used_type_string {
                imports.add_used_type(s, prop.version);
            }
            if type_string.is_ok() {
                imports.add("gobject_ffi", prop.version);
                imports.add("glib::Value", prop.version);
                imports.add("glib::StaticType", prop.version);
            }

            properties.push(prop);
        }
        if let Some(prop) = setter {
            if let Ok(ref s) = used_type_string {
                imports.add_used_type(s, prop.version);
            }
            if type_string.is_ok() {
                imports.add("gobject_ffi", prop.version);
                imports.add("glib::Value", prop.version);
            }

            if prop.bound.is_some() {
                imports.add("glib", prop.version);
                imports.add("glib::object::IsA", prop.version);
            }

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
    trampolines: &mut trampolines::Trampolines,
    obj: &GObject,
    imports: &mut Imports,
    signatures: &Signatures,
    deps: &[library::TypeId],
) -> (Option<Property>, Option<Property>, Option<signals::Info>) {
    let name = prop.name.clone();

    let prop_version = configured_properties
        .iter()
        .filter_map(|f| f.version)
        .min()
        .or(prop.version)
        .or_else(|| Some(env.config.min_cfg_version));
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

    if readable {
        let (has, version) = Signature::has_for_property(env, &check_get_func_name,
                                                         true, prop.typ, signatures, deps);
        if has && (env.is_totally_deprecated(version) || version <= prop_version) {
            readable = false;
        }
    }
    if writable {
        let (has, version) = Signature::has_for_property(env, &check_set_func_name,
                                                         false, prop.typ, signatures, deps);
        if has && (env.is_totally_deprecated(version) || version <= prop_version) {
            writable = false;
        }
    }

    let get_out_ref_mode = RefMode::of(env, prop.typ, library::ParameterDirection::Return);
    let mut set_in_ref_mode = RefMode::of(env, prop.typ, library::ParameterDirection::In);
    if set_in_ref_mode == RefMode::ByRefMut {
        set_in_ref_mode = RefMode::ByRef;
    }
    let nullable = library::Nullable(set_in_ref_mode.is_ref());
    let getter = if readable {
        Some(Property {
            name: name.clone(),
            var_name: String::new(),
            typ: prop.typ,
            is_get: true,
            func_name: get_func_name,
            nullable,
            get_out_ref_mode,
            set_in_ref_mode,
            version: prop_version,
            deprecated_version: prop.deprecated_version,
            bound: None,
        })
    } else {
        None
    };

    let setter = if writable {
        let bound = Bound::get_for_property_setter(env, &var_name, prop.typ, nullable);
        Some(Property {
            name: name.clone(),
            var_name,
            typ: prop.typ,
            is_get: false,
            func_name: set_func_name,
            nullable,
            get_out_ref_mode,
            set_in_ref_mode,
            version: prop_version,
            deprecated_version: prop.deprecated_version,
            bound,
        })
    } else {
        None
    };

    let mut used_types: Vec<String> = Vec::with_capacity(4);
    let trampoline_name = trampolines::analyze(
        env,
        &library::Signal {
            name: format!("notify::{}", name),
            parameters: Vec::new(),
            ret: library::Parameter {
                name: "".into(),
                typ: env.library
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
                async: false,
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
        trampolines,
        obj,
        &mut used_types,
        prop_version,
    );

    let notify_signal = if trampoline_name.is_ok() {
        imports.add_used_types(&used_types, prop_version);
        if generate_trait {
            imports.add("glib", prop_version);
            imports.add("glib::object::Downcast", prop_version);
        }
        imports.add("glib::signal::connect", prop_version);
        imports.add("glib::signal::SignalHandlerId", prop_version);
        imports.add("std::mem::transmute", prop_version);
        imports.add("std::boxed::Box as Box_", prop_version);
        imports.add("glib_ffi", prop_version);

        Some(signals::Info {
            connect_name: format!("connect_property_{}_notify", name_for_func),
            signal_name: format!("notify::{}", name),
            trampoline_name,
            action_emit_name: None,
            version: prop_version,
            deprecated_version: prop.deprecated_version,
            doc_hidden: false,
        })
    } else {
        None
    };

    (getter, setter, notify_signal)
}
