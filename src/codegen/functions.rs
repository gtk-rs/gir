use std::path::Path;

use env::Env;
use file_saver;
use codegen::general;
use codegen::function;

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    info!("Generate global functions");

    let functions = match env.analysis.global_functions {
        Some(ref functions) => functions,
        None => return,
    };

    let path = root_path.join("functions.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        try!(general::start_comments(w, &env.config));
        try!(general::uses(w, env, &functions.imports));

        try!(writeln!(w, ""));

        mod_rs.push("\npub mod functions;".into());

        for func_analysis in &functions.functions {
            try!(function::generate(w, env, func_analysis, false, false, 0));
        }

        Ok(())
    });
}
