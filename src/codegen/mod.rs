use std::path::Path;

use env::Env;
use config::WorkMode;
use file_saver::*;

mod function;
mod function_body_chunk;
mod general;
mod object;
mod objects;
mod parameter;
mod return_value;
mod sys;
pub mod translate_from_glib;
pub mod translate_to_glib;

pub fn generate(env: &Env) {
    match env.config.work_mode {
        WorkMode::Normal => normal_generate(env),
        WorkMode::Sys => sys::generate(env),
    }
}

fn normal_generate(env: &Env) {
    let mut mod_rs: Vec<String> = Vec::new();
    let mut traits: Vec<String> = Vec::new();
    let root_path = env.config.target_path.join("src").join("auto");

    objects::generate(env, &root_path, &mut mod_rs, &mut traits);

    generate_mod_rs(env, &root_path, mod_rs, traits);
}

pub fn generate_mod_rs(env: &Env, root_path: &Path, mod_rs: Vec<String>, traits: Vec<String>) {
    let path = root_path.join("mod.rs");
    save_to_file(path, env.config.make_backup, |w| {
        try!(general::start_comments(w, &env.config));
        try!(general::write_vec(w, &mod_rs));
        try!(writeln!(w, ""));
        try!(writeln!(w, "pub mod traits {{"));
        try!(general::write_vec(w, &traits));
        writeln!(w, "}}")
    });
}
