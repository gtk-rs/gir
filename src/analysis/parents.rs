use std::vec::Vec;

use analysis::rust_type::used_rust_type;
use env::Env;
use super::general::StatusedTypeId;
use super::imports::Imports;
use library::{Class, Interface};
use traits::*;

pub fn analyze_class(env: &Env, type_: &Class, imports: &mut Imports)
        -> Vec<StatusedTypeId> {
    let mut parents = Vec::new();

    for &parent_tid in &type_.parents {
        let parent_type = env.type_(parent_tid).to_ref_as::<Class>();
        let status = env.type_status(&parent_tid.full_name(&env.library));

        parents.push(StatusedTypeId{
            type_id: parent_tid,
            name: parent_type.name.clone(),
            status: status,
        });

        if !status.ignored() {
            used_rust_type(env, parent_tid).ok().map(|s| imports.add(s, None));
        }
    }

    parents.reverse();
    parents
}

pub fn analyze_interface(env: &Env, type_: &Interface, imports: &mut Imports)
        -> Vec<StatusedTypeId> {
    let mut parents = Vec::new();

    for &parent_tid in &type_.prereq_parents {
        let status = env.type_status(&parent_tid.full_name(&env.library));

        parents.push(StatusedTypeId{
            type_id: parent_tid,
            name: env.type_(parent_tid).get_name().into(),
            status: status,
        });

        if !status.ignored() {
            used_rust_type(env, parent_tid).ok().map(|s| imports.add(s, None));
        }
    }

    parents.reverse();
    parents
}
