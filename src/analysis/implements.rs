use std::collections::HashSet;
use std::vec::Vec;

use analysis::rust_type::used_rust_type;
use env::Env;
use super::general::StatusedTypeId;
use gobjects::*;
use library;

pub fn analyze(env: &Env, type_: &library::Class, used_types: &mut HashSet<String>)
    -> Vec<StatusedTypeId> {
    let mut implements = Vec::new();

    for &interface_tid in &type_.implements {
        let status = env.type_status(&interface_tid.full_name(&env.library));

        if status == GStatus::Ignore { continue }

        let interface_type = env.type_(interface_tid).to_interface();

        implements.push(StatusedTypeId{
            type_id: interface_tid,
            name: interface_type.name.clone(),
            status: status,
        });
        let _ = used_rust_type(env, interface_tid)
            .map(|s| used_types.insert(s));
    }
    implements
}
