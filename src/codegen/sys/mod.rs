use env::Env;

mod build;
mod cargo_toml;
mod fields;
mod functions;
mod lib_;
mod statics;
mod tests;
pub mod ffi_type;

pub fn generate(env: &Env) {
    lib_::generate(env);
    build::generate(env);
    cargo_toml::generate(env);
    tests::generate(env);
}
