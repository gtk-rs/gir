use analysis::namespaces;
use codegen::general::version_condition_string;
use env::Env;
use library::*;
use std::path::Path;

pub fn generate(env: &Env, _root_path: &Path, mod_rs: &mut Vec<String>) {
    let configs = env.config.objects.values()
        .filter(|c| {
            c.status.need_generate() &&
                c.type_id.map_or(false, |tid| tid.ns_id == namespaces::MAIN)
        });
    mod_rs.push(String::from(""));
    for config in configs {
        if let Type::Bitfield(ref flags) = *env.library.type_(config.type_id.unwrap()) {
            if let Some (cfg) = version_condition_string(env, flags.version, false, 0) {
                mod_rs.push(cfg);
            }
            mod_rs.push(format!("pub use ffi::{} as {};", flags.c_type, flags.name));
        }
    }
}
