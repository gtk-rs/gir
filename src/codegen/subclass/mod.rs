use env::Env;
use codegen::general;
use std::path::Path;

use file_saver::*;

mod object;
mod statics;
mod class_impls;
mod class_impl;
mod virtual_methods;
mod virtual_method_body_chunks;


use codegen::generate_single_version_file;

pub fn generate(env: &Env) {
    info!("Generating subclasssing traits {:?}", env.config.target_path);

    let root_path = env.config.target_path.join("src").join("auto");
    let mut lib_rs: Vec<String> = Vec::new();
    let mut modules: Vec<String> = Vec::new();

    generate_single_version_file(env);

    class_impls::generate(env, &root_path, &mut lib_rs);

    generate_lib_rs(env, &root_path, &lib_rs);

    // lib_::generate(env);
    // build::generate(env);
    // let crate_name = cargo_toml::generate(env);
    // tests::generate(env, &crate_name);
}


pub fn generate_lib_rs(env: &Env, root_path: &Path, lib_rs: &[String]) {

    let path = root_path.join("lib.rs");
    save_to_file(path, env.config.make_backup, |w| {
        try!(general::start_comments(w, &env.config));
        try!(writeln!(w));
        try!(statics::generate_extern_crates(w, env));
        try!(writeln!(w));
        general::write_vec(w, lib_rs)
    });
}
