use std::io::{Result, Write};

use env::Env;
use file_saver::save_to_file;

pub fn generate(env: &Env) {
    println!("generating sys build script for {}", env.config.library_name);

    let path = env.config.target_path.join("build.rs");

    println!("Generating file {:?}", path);
    save_to_file(&path, env.config.make_backup,
        |w| generate_build_script(w));
}

fn generate_build_script(w: &mut Write) -> Result<()> {
    writeln!(w, "{}", "fn main() {{ }}")
}
