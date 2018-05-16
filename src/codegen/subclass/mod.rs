use env::Env;
mod objects;
mod object;

use codegen::generate_single_version_file;

pub fn generate(env: &Env) {
    info!("Generating subclasssing traits {:?}", env.config.subclass_target_path);

    let root_path = env.config.subclass_target_path.join("src").join("auto");
    let mut mod_rs: Vec<String> = Vec::new();
    let mut traits: Vec<String> = Vec::new();

    generate_single_version_file(env);
    objects::generate(env, &root_path, &mut mod_rs, &mut traits);

    // lib_::generate(env);
    // build::generate(env);
    // let crate_name = cargo_toml::generate(env);
    // tests::generate(env, &crate_name);
}
