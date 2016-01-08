use std::io::{Result, Write};

use analysis;
use analysis::special_functions::Type;
use env::Env;
use super::{function, general, trait_impls};

pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::record::Info) -> Result<()>{
    let type_ = analysis.type_(&env.library);

    try!(general::start_comments(w, &env.config));
    try!(general::uses(w, &analysis.imports, &env.config.library_name, env.config.min_cfg_version));

    let copy_fn = analysis.specials.get(&Type::Copy).expect("No copy function for record");
    let free_fn = analysis.specials.get(&Type::Free).expect("No free function for record");
    try!(general::define_boxed_type(w, &analysis.name, &type_.c_type, copy_fn, free_fn));
    try!(writeln!(w, ""));
    try!(writeln!(w, "impl {} {{", analysis.name));

    for func_analysis in &analysis.functions {
        try!(function::generate(w, env, func_analysis, false, false, 1));
    }

    try!(writeln!(w, "}}"));

    try!(trait_impls::generate(w, &analysis.name, &analysis.functions, &analysis.specials));

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
