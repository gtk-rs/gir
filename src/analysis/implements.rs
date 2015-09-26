use std::vec::Vec;

use analysis::rust_type::used_rust_type;
use env::Env;
use super::general::StatusedTypeId;
use super::imports::Imports;
use library;

pub fn analyze(env: &Env, type_: &library::Class, imports: &mut Imports)
    -> Vec<StatusedTypeId> {
    let mut implements = Vec::new();

    for &interface_tid in &type_.implements {
        let status = env.type_status(&interface_tid.full_name(&env.library));

        if status.ignored() { continue }

        let name = env.type_(interface_tid).get_name();

        implements.push(StatusedTypeId{
            type_id: interface_tid,
            name: name,
            status: status,
        });
        let _ = used_rust_type(env, interface_tid)
            .map(|s| imports.add(s, None));
    }
    implements
}
