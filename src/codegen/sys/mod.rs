use env::Env;
use super::generate_single_version_file;

mod build;
mod cargo_toml;
pub mod ffi_type;
mod fields;
mod functions;
mod lib_;
mod statics;
mod tests;

pub fn generate(env: &Env) {
    if let Some(ref version_path) = env.config.single_version_file {
        generate_single_version_file(env, version_path);
    }
    lib_::generate(env);
    build::generate(env);
    let crate_name = cargo_toml::generate(env);
    tests::generate(env, &crate_name);
}
