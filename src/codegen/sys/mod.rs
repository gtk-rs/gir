use codegen::generate_single_version_file;
use env::Env;

mod build;
mod cargo_toml;
pub mod ffi_type;
pub mod fields;
pub mod functions;
mod lib_;
pub mod statics;
mod tests;

pub fn generate(env: &Env) {
    generate_single_version_file(env);
    lib_::generate(env);
    build::generate(env);
    let crate_name = cargo_toml::generate(env);
    tests::generate(env, &crate_name);
}
