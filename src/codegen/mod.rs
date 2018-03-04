use std::path::Path;

use env::Env;
use config::WorkMode;
use file_saver::*;

mod child_properties;
mod doc;
mod enums;
mod constants;
mod flags;
mod alias;
pub mod function;
mod function_body_chunk;
mod functions;
mod general;
mod object;
mod objects;
mod parameter;
mod properties;
mod property_body;
mod record;
mod records;
mod return_value;
mod signal;
mod signal_body;
mod sys;
mod trait_impls;
mod trampoline;
mod trampoline_from_glib;
mod trampoline_to_glib;
pub mod translate_from_glib;
pub mod translate_to_glib;

pub fn generate(env: &Env) {
    match env.config.work_mode {
        WorkMode::Normal => normal_generate(env),
        WorkMode::Sys => sys::generate(env),
        WorkMode::Doc => doc::generate(env),
        WorkMode::DisplayNotBound => {}
    }
}

fn normal_generate(env: &Env) {
    let mut mod_rs: Vec<String> = Vec::new();
    let mut traits: Vec<String> = Vec::new();
    let root_path = env.config.target_path.join("src").join("auto");

    if env.config.single_version_file {
        generate_single_version_file(env, &root_path);
    }

    objects::generate(env, &root_path, &mut mod_rs, &mut traits);
    records::generate(env, &root_path, &mut mod_rs);
    enums::generate(env, &root_path, &mut mod_rs);
    flags::generate(env, &root_path, &mut mod_rs);
    alias::generate(env, &root_path, &mut mod_rs);
    functions::generate(env, &root_path, &mut mod_rs);
    constants::generate(env, &root_path, &mut mod_rs);

    generate_mod_rs(env, &root_path, &mod_rs, &traits);
}

pub fn generate_mod_rs(env: &Env, root_path: &Path, mod_rs: &[String], traits: &[String]) {
    let path = root_path.join("mod.rs");
    save_to_file(path, env.config.make_backup, |w| {
        try!(general::start_comments(w, &env.config));
        try!(general::write_vec(w, mod_rs));
        try!(writeln!(w, ""));
        try!(writeln!(w, "#[doc(hidden)]"));
        try!(writeln!(w, "pub mod traits {{"));
        try!(general::write_vec(w, traits));
        writeln!(w, "}}")
    });
}

pub fn generate_single_version_file(env: &Env, root_path: &Path) {
    let path = root_path.join("versions.txt");
    save_to_file(path, env.config.make_backup, |w| {
        general::single_version_file(w, &env.config)
    });
}
