use std::path::Path;

use crate::env::Env;
use crate::file_saver;
use crate::codegen::general;
use crate::codegen::function;

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    info!("Generate global functions");

    let functions = match env.analysis.global_functions {
        Some(ref functions) => functions,
        None => return,
    };

    let path = root_path.join("functions.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        general::start_comments(w, &env.config)?;
        general::uses(w, env, &functions.imports)?;

        writeln!(w)?;

        mod_rs.push("\npub mod functions;".into());

        for func_analysis in &functions.functions {
            function::generate(w, env, func_analysis, false, false, 0)?;
        }

        Ok(())
    });
}
