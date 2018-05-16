use env::Env;

mod build;
mod cargo_toml;
pub mod ffi_type;
mod fields;
mod functions;
mod lib_;
mod statics;
mod tests;

pub fn generate(env: &Env) {
    info!("Generating subclasssing traits {:?}", env.config.subclass_target_path);

    // generate_single_version_file(env);
    // lib_::generate(env);
    // build::generate(env);
    // let crate_name = cargo_toml::generate(env);
    // tests::generate(env, &crate_name);
}
