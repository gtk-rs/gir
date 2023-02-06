use super::{general::StatusedTypeId, imports::Imports};
use crate::{
    analysis::{namespaces, rust_type::RustType},
    env::Env,
    library::TypeId,
    version::Version,
};

pub fn analyze(
    env: &Env,
    type_id: TypeId,
    version: Option<Version>,
    imports: &mut Imports,
    add_parent_types_import: bool,
) -> Vec<StatusedTypeId> {
    let mut parents = Vec::new();
    let gobject_id = env.library.find_type(0, "GObject.Object").unwrap();

    for &super_tid in env.class_hierarchy.supertypes(type_id) {
        // skip GObject, it's inherited implicitly
        if super_tid == gobject_id {
            continue;
        }

        let status = env.type_status(&super_tid.full_name(&env.library));

        parents.push(StatusedTypeId {
            type_id: super_tid,
            name: env.library.type_(super_tid).get_name(),
            status,
        });

        if !status.ignored() && super_tid.ns_id == namespaces::MAIN && !add_parent_types_import {
            if let Ok(rust_type) = RustType::try_new(env, super_tid) {
                let full_name = super_tid.full_name(&env.library);
                if let Some(parent_version) = env
                    .analysis
                    .objects
                    .get(&full_name)
                    .and_then(|info| info.version)
                {
                    if Some(parent_version) > version && parent_version > env.config.min_cfg_version
                    {
                        for import in rust_type.into_used_types() {
                            imports.add_with_version(
                                &format!("crate::{import}"),
                                Some(parent_version),
                            );
                        }
                    } else {
                        for import in rust_type.into_used_types() {
                            imports.add(&format!("crate::{import}"));
                        }
                    }
                } else {
                    for import in rust_type.into_used_types() {
                        imports.add(&format!("crate::{import}"));
                    }
                }
            }
        }
    }

    parents
}

pub fn dependencies(env: &Env, type_id: TypeId) -> Vec<TypeId> {
    let mut parents = Vec::new();
    let gobject_id = match env.library.find_type(0, "GObject.Object") {
        Some(gobject_id) => gobject_id,
        None => TypeId::tid_none(),
    };

    for &super_tid in env.class_hierarchy.supertypes(type_id) {
        // skip GObject, it's inherited implicitly
        if super_tid == gobject_id {
            continue;
        }

        let status = env.type_status(&super_tid.full_name(&env.library));

        if status.need_generate() {
            parents.push(super_tid);
        }
    }

    parents
}
