use std::io::Write;
use std::path::{Path, PathBuf};

use analysis;
use env::Env;
use file_saver::*;
use nameutil::*;
use super::general;

pub fn generate(env: &Env) {
    let root_path = PathBuf::from(&env.config.target_path).join("src").join("auto");

    let mut mod_rs: Vec<String> = Vec::new();
    let mut traits: Vec<String> = Vec::new();

    for obj in env.config.objects.values() {
        if !obj.status.need_generate() {
            continue;
        }

        println!("Analyzing {:?}", obj.name);
        let class_analysis = analysis::widget::new(env, obj);
        if class_analysis.has_ignored_parents {
            println!("Skipping {:?}, it has ignored parents", obj.name);
            continue;
        }

        let path = root_path.join(file_name(&class_analysis.full_name));
        println!("Generating file {:?}", path);

        save_to_file(path, env.config.make_backup,
            &mut |w| super::widget::generate(w, env, &class_analysis));

        let mod_name = module_name(split_namespace_name(&class_analysis.full_name).1);
        super::widget::generate_reexports(env, &class_analysis, &mod_name, &mut mod_rs,
            &mut traits);
    }

    generate_mod_rs(env, &root_path, mod_rs, traits);
}

fn generate_mod_rs(env: &Env, root_path: &Path, mod_rs: Vec<String>, traits: Vec<String>) {
    let path = root_path.join("mod.rs");
    save_to_file(path, env.config.make_backup, &mut |w| {
        try!(general::start_comments(w, &env.config));
        try!(general::write_vec(w, &mod_rs));
        try!(writeln!(w, ""));
        try!(writeln!(w, "pub mod traits {{"));
        try!(general::write_vec(w, &traits));
        writeln!(w, "}}")
    });
}
