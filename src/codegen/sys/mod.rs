use crate::{codegen::generate_single_version_file, env::Env, version::Version};
use std::collections::BTreeMap;

mod build;
mod cargo_toml;
pub mod ffi_type;
mod fields;
mod functions;
mod lib_;
mod statics;
mod tests;

pub fn generate(env: &Env) {
    generate_single_version_file(env);
    lib_::generate(env);
    let crate_name = cargo_toml::generate(env);
    let has_abi_tests = tests::generate(env, &crate_name);
    build::generate(env, has_abi_tests);
}

pub fn collect_versions(env: &Env) -> BTreeMap<Version, Version> {
    let mut versions: BTreeMap<Version, Version> = env
        .namespaces
        .main()
        .versions
        .iter()
        .filter(|v| **v > env.config.min_cfg_version)
        .map(|v| (*v, *v))
        .collect();

    for v in &env.config.extra_versions {
        versions.insert(*v, *v);
    }

    for (version, lib_version) in &env.config.lib_version_overrides {
        versions.insert(*version, *lib_version);
    }

    versions
}
