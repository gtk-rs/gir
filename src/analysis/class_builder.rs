use std::collections::HashSet;

use crate::{
    analysis::{
        bounds::Bounds,
        imports::Imports,
        properties::{get_property_ref_modes, Property},
        rust_type::RustType,
    },
    config::{self, GObject},
    env::Env,
    library,
    traits::*,
};

pub fn analyze(
    env: &Env,
    props: &[library::Property],
    type_tid: library::TypeId,
    obj: &GObject,
    imports: &mut Imports,
) -> Vec<(Vec<Property>, library::TypeId)> {
    if !obj.generate_builder {
        return Vec::new();
    }

    let mut names = HashSet::<String>::new();
    let mut builder_properties = vec![(
        analyze_properties(env, type_tid, props, obj, imports, &mut names),
        type_tid,
    )];

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

        let new_builder_properties = (
            analyze_properties(
                env,
                super_tid,
                super_properties,
                super_obj,
                imports,
                &mut names,
            ),
            super_tid,
        );
        builder_properties.push(new_builder_properties);
    }

    builder_properties
}

fn analyze_properties(
    env: &Env,
    type_tid: library::TypeId,
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
        if !configured_properties
            .iter()
            .all(|f| f.status.need_generate())
        {
            continue;
        }

        if env.is_totally_deprecated(Some(type_tid.ns_id), prop.deprecated_version) {
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
    let prop_version = configured_properties
        .iter()
        .filter_map(|f| f.version)
        .min()
        .or(prop.version);

    let for_builder = prop.construct_only || prop.construct || prop.writable;
    if !for_builder {
        return None;
    }
    let imports = &mut imports.with_defaults(prop_version, &None);
    let rust_type_res = RustType::try_new(env, prop.typ);
    if let Ok(ref rust_type) = rust_type_res {
        if !rust_type.as_str().contains("GString") {
            imports.add_used_types(rust_type.used_types());
        }
    }

    let (get_out_ref_mode, set_in_ref_mode, nullable) = get_property_ref_modes(env, prop);

    let mut bounds = Bounds::default();
    if let Some(bound) = Bounds::type_for(env, prop.typ) {
        imports.add("glib::prelude::*");
        bounds.add_parameter(&prop.name, &rust_type_res.into_string(), bound, false);
    }

    Some(Property {
        name: prop.name.clone(),
        var_name: String::new(),
        typ: prop.typ,
        is_get: false,
        func_name: String::new(),
        func_name_alias: None,
        nullable,
        get_out_ref_mode,
        set_in_ref_mode,
        set_bound: None,
        bounds,
        version: prop_version,
        deprecated_version: prop.deprecated_version,
    })
}
