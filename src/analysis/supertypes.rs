use std::vec::Vec;

use analysis::rust_type::used_rust_type;
use env::Env;
use super::general::StatusedTypeId;
use super::imports::Imports;
use library::TypeId;

pub fn analyze(env: &Env, type_id: TypeId, imports: &mut Imports) -> Vec<StatusedTypeId> {
    let mut parents = Vec::new();

    for &super_tid in env.class_hierarchy.supertypes(type_id) {
        let status = env.type_status(&super_tid.full_name(&env.library));

        parents.push(StatusedTypeId{
            type_id: super_tid,
            name: env.library.type_(super_tid).get_name(),
            status: status,
        });

        if !status.ignored() {
            used_rust_type(env, super_tid).ok().map(|s| imports.add(&s, None));
        }
    }

    parents
}
