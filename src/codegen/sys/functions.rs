use std::path::*;
use std::io::{Result, Write};

use env::Env;
use file_saver::*;
use nameutil::*;
use super::super::general;

pub fn generate(env: &Env) {
    println!("generating sys for {}", env.config.library_name);

    let path =  PathBuf::from(&env.config.target_path)
        .join(file_name_sys(&env.config.library_name, "lib"));
    println!("Generating file {:?}", path);

    save_to_file(path, &mut |w| generate_funcs(w, env));
}

fn generate_funcs<W: Write>(w: &mut W, env: &Env) -> Result<()>{
    try!(general::start_comments(w, &env.config));

    Ok(())
}
