use log::warn;

use crate::{
    analysis::{
        bounds::{Bounds, PropertyBound},
        imports::Imports,
        ref_mode::RefMode,
        rust_type::RustType,
        signals,
        signatures::{Signature, Signatures},
        trampolines,
    },
    config::{self, gobjects::GStatus, GObject, PropertyGenerateFlags},
    env::Env,
    library, nameutil,
    traits::*,
    version::Version,
};

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub var_name: String,
    pub typ: library::TypeId,
    pub is_get: bool,
    pub func_name: String,
    pub func_name_alias: Option<String>,
    pub nullable: bool,
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
    supertypes_props: &[&library::Property],
    type_tid: library::TypeId,
    generate_trait: bool,
    is_fundamental: bool,
    obj: &GObject,
    imports: &mut Imports,
    signatures: &Signatures,
    deps: &[library::TypeId],
    functions: &[crate::analysis::functions::Info],
) -> (Vec<Property>, Vec<signals::Info>) {
    let mut properties = Vec::new();
    let mut notify_signals = Vec::new();

    for prop in props {
        let configured_properties = obj.properties.matched(&prop.name);
        if !configured_properties
            .iter()
            .all(|f| f.status.need_generate())
        {
            continue;
        }

        if env.is_totally_deprecated(Some(type_tid.ns_id), prop.deprecated_version) {
            continue;
        }

        if supertypes_props
            .iter()
            .any(|p| p.name == prop.name && p.typ == prop.typ)
        {
            continue;
        }

        let (getter, setter, notify_signal) = analyze_property(
            env,
            prop,
            type_tid,
            &configured_properties,
            generate_trait,
            is_fundamental,
            obj,
            imports,
            signatures,
            deps,
            functions,
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
    is_fundamental: bool,
    obj: &GObject,
    imports: &mut Imports,
    signatures: &Signatures,
    deps: &[library::TypeId],
    functions: &[crate::analysis::functions::Info],
) -> (Option<Property>, Option<Property>, Option<signals::Info>) {
    let type_name = type_tid.full_name(&env.library);
    let name = prop.name.clone();

    let prop_version = configured_properties
        .iter()
        .filter_map(|f| f.version)
        .min()
        .or(prop.version)
        .or(Some(env.config.min_cfg_version));
    let generate = configured_properties.iter().find_map(|f| f.generate);
    let generate_set = generate.is_some();
    let generate = generate.unwrap_or_else(PropertyGenerateFlags::all);

    let imports = &mut imports.with_defaults(prop_version, &None);
    imports.add("glib::translate::*");

    let type_string = RustType::try_new(env, prop.typ);
    let name_for_func = nameutil::signal_to_snake(&name);

    let mut get_prop_name = Some(format!("get_property_{name_for_func}"));

    let bypass_auto_rename = configured_properties.iter().any(|f| f.bypass_auto_rename);
    let (check_get_func_names, mut get_func_name) = if bypass_auto_rename {
        (
            vec![format!("get_{name_for_func}")],
            get_prop_name.take().expect("defined 10 lines above"),
        )
    } else {
        get_func_name(&name_for_func, prop.typ == library::TypeId::tid_bool())
    };

    let mut set_func_name = format!("set_{name_for_func}");
    let mut set_prop_name = Some(format!("set_property_{name_for_func}"));

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
        for check_get_func_name in check_get_func_names {
            let (has, version) = Signature::has_for_property(
                env,
                &check_get_func_name,
                true,
                prop.typ,
                signatures,
                deps,
            );
            if has {
                // There is a matching get func
                if env.is_totally_deprecated(Some(type_tid.ns_id), version)
                    || version <= prop_version
                {
                    // And its availability covers the property's availability
                    // => don't generate the get property.
                    readable = false;
                } else {
                    // The property is required in earlier versions than the getter
                    // => we need both, but there will be a name clash due to auto renaming
                    // => keep the get_property name.
                    if let Some(get_prop_name) = get_prop_name.take() {
                        get_func_name = get_prop_name;
                    }
                }
            }
        }
    }
    if writable {
        let (has, version) =
            Signature::has_for_property(env, &set_func_name, false, prop.typ, signatures, deps);
        if has {
            // There is a matching set func
            if env.is_totally_deprecated(Some(type_tid.ns_id), version) || version <= prop_version {
                // And its availability covers the property's availability
                // => don't generate the set property.
                writable = false;
            } else {
                // The property is required in earlier versions than the setter
                // => we need both, but there will be a name clash due to auto renaming
                // => keep the set_property name.
                if let Some(set_prop_name) = set_prop_name.take() {
                    set_func_name = set_prop_name;
                }
            }
        }
    }

    let (get_out_ref_mode, set_in_ref_mode, nullable) = get_property_ref_modes(env, prop);

    let getter_func = functions.iter().find(|f| {
        f.get_property.as_ref() == Some(&prop.name) && Some(&f.name) == prop.getter.as_ref()
    });
    let setter_func = functions.iter().find(|f| {
        f.set_property.as_ref() == Some(&prop.name) && Some(&f.name) == prop.setter.as_ref()
    });

    let has_getter =
        getter_func.is_some_and(|g| matches!(g.status, GStatus::Generate | GStatus::Manual));
    let has_setter =
        setter_func.is_some_and(|s| matches!(s.status, GStatus::Generate | GStatus::Manual));

    let getter = if readable && (!has_getter || prop.version < getter_func.and_then(|g| g.version))
    {
        if let Ok(rust_type) = RustType::builder(env, prop.typ)
            .direction(library::ParameterDirection::Out)
            .try_build()
        {
            imports.add_used_types(rust_type.used_types());
        }
        if type_string.is_ok() {
            imports.add("glib::prelude::*");
        }

        let mut getter_version = prop_version;
        if has_getter {
            let getter = getter_func.unwrap();
            get_func_name = getter.new_name.as_ref().unwrap_or(&getter.name).to_string();
            get_prop_name = Some(getter.name.clone());
            getter_version = getter.version.map(|mut g| {
                g.as_opposite();
                g
            });
        }

        Some(Property {
            name: name.clone(),
            var_name: nameutil::mangle_keywords(&*name_for_func).into_owned(),
            typ: prop.typ,
            is_get: true,
            func_name: get_func_name,
            func_name_alias: get_prop_name,
            nullable,
            get_out_ref_mode,
            set_in_ref_mode,
            set_bound: None,
            bounds: Bounds::default(),
            version: getter_version,
            deprecated_version: prop.deprecated_version,
        })
    } else {
        None
    };

    let setter = if writable && (!has_setter || prop.version < setter_func.and_then(|s| s.version))
    {
        if let Ok(rust_type) = RustType::builder(env, prop.typ)
            .direction(library::ParameterDirection::In)
            .try_build()
        {
            imports.add_used_types(rust_type.used_types());
        }
        if type_string.is_ok() {
            imports.add("glib::prelude::*");
        }
        let set_bound = PropertyBound::get(env, prop.typ);
        if type_string.is_ok() && set_bound.is_some() {
            imports.add("glib::prelude::*");
            if !nullable {
                // TODO: support non-nullable setter if found any
                warn!(
                    "Non nullable setter for property generated as nullable \"{}.{}\"",
                    type_name, name
                );
            }
        }

        let mut setter_version = prop_version;
        if has_setter {
            let setter = setter_func.unwrap();
            set_func_name = setter.new_name.as_ref().unwrap_or(&setter.name).to_string();
            set_prop_name = Some(setter.name.clone());
            setter_version = setter.version.map(|mut s| {
                s.as_opposite();
                s
            });
        }

        Some(Property {
            name: name.clone(),
            var_name: nameutil::mangle_keywords(&*name_for_func).into_owned(),
            typ: prop.typ,
            is_get: false,
            func_name: set_func_name,
            func_name_alias: set_prop_name,
            nullable,
            get_out_ref_mode,
            set_in_ref_mode,
            set_bound,
            bounds: Bounds::default(),
            version: setter_version,
            deprecated_version: prop.deprecated_version,
        })
    } else {
        None
    };

    if !generate_trait && (writable || readable || notifiable) {
        imports.add("glib::prelude::*");
    }

    let notify_signal = if notifiable {
        let mut used_types: Vec<String> = Vec::with_capacity(4);
        let tid = env
            .library
            .find_type(library::INTERNAL_NAMESPACE, "none")
            .unwrap();
        let trampoline = trampolines::analyze(
            env,
            &library::Signal {
                name: format!("notify::{name}"),
                parameters: Vec::new(),
                ret: library::Parameter::none(tid),
                is_action: false,
                is_detailed: false, /* well, technically this *is* an instance of a detailed
                                     * signal, but we "pre-detailed" it */
                version: prop_version,
                deprecated_version: prop.deprecated_version,
                doc: None,
                doc_deprecated: None,
            },
            type_tid,
            generate_trait,
            is_fundamental,
            &[],
            obj,
            &mut used_types,
            prop_version,
        );

        if trampoline.is_ok() {
            imports.add_used_types(&used_types);
            if generate_trait {
                imports.add("glib::prelude::*");
            }
            imports.add("glib::signal::{connect_raw, SignalHandlerId}");
            imports.add("std::boxed::Box as Box_");

            Some(signals::Info {
                connect_name: format!("connect_{name_for_func}_notify"),
                signal_name: format!("notify::{name}"),
                trampoline,
                action_emit_name: None,
                version: prop_version,
                deprecated_version: prop.deprecated_version,
                doc_hidden: false,
                is_detailed: false, // see above comment
                generate_doc: obj.generate_doc,
            })
        } else {
            None
        }
    } else {
        None
    };

    (getter, setter, notify_signal)
}

/// Returns (the list of get functions to check, the desired get function name).
fn get_func_name(prop_name: &str, is_bool_getter: bool) -> (Vec<String>, String) {
    let get_rename_res = getter_rules::try_rename_getter_suffix(prop_name, is_bool_getter);
    match get_rename_res {
        Ok(new_name) => {
            let new_name = new_name.unwrap();
            let mut check_get_func_names = vec![
                format!("get_{prop_name}"),
                prop_name.to_string(),
                format!("get_{new_name}"),
                new_name.clone(),
            ];

            if is_bool_getter {
                check_get_func_names.push(format!("is_{prop_name}"));
                check_get_func_names.push(format!("is_{new_name}"));
            }
            (check_get_func_names, new_name)
        }
        Err(_) => {
            let mut check_get_func_names = vec![format!("get_{prop_name}"), prop_name.to_string()];

            // Name is reserved
            let get_func_name = if is_bool_getter {
                let get_func_name = format!("is_{prop_name}");
                check_get_func_names.push(get_func_name.clone());
                get_func_name
            } else {
                format!("get_{prop_name}")
            };
            (check_get_func_names, get_func_name)
        }
    }
}

pub fn get_property_ref_modes(env: &Env, prop: &library::Property) -> (RefMode, RefMode, bool) {
    let get_out_ref_mode = RefMode::of(env, prop.typ, library::ParameterDirection::Return);
    let mut set_in_ref_mode = RefMode::of(env, prop.typ, library::ParameterDirection::In);
    if set_in_ref_mode == RefMode::ByRefMut {
        set_in_ref_mode = RefMode::ByRef;
    }
    let nullable = set_in_ref_mode.is_ref();
    (get_out_ref_mode, set_in_ref_mode, nullable)
}
