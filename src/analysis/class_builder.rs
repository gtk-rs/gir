use crate::{
    analysis::{
        imports::Imports,
        properties::{get_property_ref_modes, Property},
        rust_type::*,
    },
    config::{self, GObject},
    env::Env,
    library,
    traits::*,
};
use std::collections::HashSet;

pub fn analyze(
    env: &Env,
    props: &[library::Property],
    type_tid: library::TypeId,
    obj: &GObject,
    imports: &mut Imports,
) -> Vec<Property> {
    if !obj.generate_builder {
        return Vec::new();
    }

    let mut names = HashSet::<String>::new();
    let mut builder_properties = analyze_properties(env, props, obj, imports, &mut names);

    for &super_tid in env.class_hierarchy.supertypes(type_tid) {
        let type_ = env.type_(super_tid);

        let super_properties = match type_ {
            library::Type::Class(class) => &class.properties,
            library::Type::Interface(iface) => &iface.properties,
            _ => continue,
        };
        let super_obj =
            if let Some(super_obj) = env.config.objects.get(&super_tid.full_name(&env.library)) {
                super_obj
            } else {
                continue;
            };

        let new_builder_properties =
            analyze_properties(env, super_properties, super_obj, imports, &mut names);
        builder_properties.extend(new_builder_properties);
    }

    builder_properties
}

fn analyze_properties(
    env: &Env,
    props: &[library::Property],
    obj: &GObject,
    imports: &mut Imports,
    names: &mut HashSet<String>,
) -> Vec<Property> {
    let mut builder_properties = Vec::new();

    for prop in props {
        if names.contains(&prop.name) {
            continue;
        }
        let configured_properties = obj.properties.matched(&prop.name);
        if configured_properties.iter().any(|f| f.ignore) {
            continue;
        }

        if env.is_totally_deprecated(prop.deprecated_version) {
            continue;
        }
        let builder = analyze_property(env, prop, &configured_properties, imports);
        if let Some(builder) = builder {
            builder_properties.push(builder);
            names.insert(prop.name.clone());
        }
    }

    builder_properties
}

fn analyze_property(
    env: &Env,
    prop: &library::Property,
    configured_properties: &[&config::properties::Property],
    imports: &mut Imports,
) -> Option<Property> {
    //let name = prop.name.clone();

    let prop_version = configured_properties
        .iter()
        .filter_map(|f| f.version)
        .min()
        .or(prop.version)
        .or_else(|| Some(env.config.min_cfg_version));

    let for_builder = prop.construct_only || prop.construct || prop.writable;
    if !for_builder {
        return None;
    }
    if let Ok(ref s) = used_rust_type(env, prop.typ, false) {
        if !s.contains("GString") {
            imports.add_used_type_with_version(s, prop_version);
        }
    }

    let (get_out_ref_mode, set_in_ref_mode, nullable) = get_property_ref_modes(env, prop);

    Some(Property {
        name: prop.name.clone(),
        var_name: String::new(),
        typ: prop.typ,
        is_get: false,
        func_name: String::new(),
        nullable,
        get_out_ref_mode,
        set_in_ref_mode,
        set_bound: None,
        version: prop_version,
        deprecated_version: prop.deprecated_version,
    })
}
