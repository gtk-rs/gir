use std::io::{Result, Write};

use analysis;
use env::Env;
use super::{function, general};

pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::record::Info) -> Result<()>{
    let type_ = analysis.type_(&env.library);

    try!(general::start_comments(w, &env.config));
    try!(general::uses(w, &analysis.imports, &env.config.library_name, env.config.min_cfg_version));

    //TODO: get special function names
    try!(general::define_boxed_type(w, &analysis.name, &type_.c_type, "xxx_copy", "xxx_free"));
    try!(writeln!(w, ""));
    try!(writeln!(w, "impl {} {{", analysis.name));

    for func_analysis in &analysis.functions {
        try!(function::generate(w, env, func_analysis, false, false, 1));
    }

    try!(writeln!(w, "}}"));

    Ok(())
}

pub fn generate_reexports(env: &Env, analysis: &analysis::record::Info, module_name: &str,
        contents: &mut Vec<String>) {
    let version_cfg = general::version_condition_string(&env.config.library_name,
        env.config.min_cfg_version, analysis.version, false, 0);
    let cfg = match version_cfg {
        Some(s) => format!("{}\n", s),
        None => "".into(),
    };
    contents.push(format!(""));
    contents.push(format!("{}mod {};", cfg, module_name));
    contents.push(format!("{}pub use self::{}::{};", cfg, module_name, analysis.name));
}
