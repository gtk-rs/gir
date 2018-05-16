use env::Env;


pub fn generate(env: &Env) {
    info!("Generating subclasssing traits {:?}", env.config.subclass_target_path);

    // generate_single_version_file(env);
    // lib_::generate(env);
    // build::generate(env);
    // let crate_name = cargo_toml::generate(env);
    // tests::generate(env, &crate_name);
}
