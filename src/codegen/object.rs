use std::io::{Result, Write};

use analysis;
use analysis::general::StatusedTypeId;
use env::Env;
use super::{function, general};

pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::object::Info) -> Result<()>{
    let implements: Vec<&StatusedTypeId> = analysis.parents.iter()
        .chain(analysis.implements.iter())
        .collect();
    try!(general::start_comments(w, &env.config));
    try!(general::uses(w, &analysis.imports, &env.config.library_name, env.config.min_cfg_version));
    try!(general::define_object_type(w, &analysis.name, &analysis.c_type, &analysis.get_type,
        &implements));

    if generate_inherent(analysis) {
        try!(writeln!(w, ""));
        try!(writeln!(w, "impl {} {{", analysis.name));
        for func_analysis in &analysis.constructors() {
            try!(function::generate(w, env, func_analysis, false, false, 1));
        }

        if !generate_trait(analysis) {
            for func_analysis in &analysis.methods() {
                try!(function::generate(w, env, func_analysis, false, false, 1));
            }
        }

        for func_analysis in &analysis.functions() {
            try!(function::generate(w, env, func_analysis, false, false, 1));
        }
        try!(writeln!(w, "}}"));
    }

    if generate_trait(analysis) {
        try!(writeln!(w, ""));
        try!(writeln!(w, "pub trait {}Ext {{", analysis.name));
        for func_analysis in &analysis.methods() {
            try!(function::generate(w, env, func_analysis, true, true, 1));
        }
        try!(writeln!(w, "}}"));

        try!(writeln!(w, ""));
        try!(writeln!(w, "impl<O: Upcast<{}>> {}Ext for O {{", analysis.name, analysis.name));
        for func_analysis in &analysis.methods() {
            try!(function::generate(w, env, func_analysis, true, false, 1));
        }
        try!(writeln!(w, "}}"));
    }

    Ok(())
}

fn generate_inherent(analysis: &analysis::object::Info) -> bool {
    analysis.has_constructors || analysis.has_functions || !analysis.has_children
}

fn generate_trait(analysis: &analysis::object::Info) -> bool {
    analysis.has_children
}

pub fn generate_reexports(env: &Env, analysis: &analysis::object::Info, module_name: &str,
        contents: &mut Vec<String>, traits: &mut Vec<String>) {
    let version_cfg = general::version_condition_string(&env.config.library_name,
        env.config.min_cfg_version, analysis.version, false, 0);
    let (cfg, cfg_1) = match version_cfg {
        Some(s) => (format!("{}\n", s), format!("\t{}\n", s)),
        None => ("".into(), "".into()),
    };
    contents.push(format!(""));
    contents.push(format!("{}mod {};", cfg, module_name));
    contents.push(format!("{}pub use self::{}::{};", cfg, module_name, analysis.name));
    if generate_trait(analysis) {
        contents.push(format!("{}pub use self::{}::{}Ext;", cfg, module_name, analysis.name));
        traits.push(format!("{}\tpub use super::{}Ext;", cfg_1, analysis.name));
    }
}
