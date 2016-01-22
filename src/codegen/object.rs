use std::io::{Result, Write};

use analysis;
use analysis::general::StatusedTypeId;
use env::Env;
use super::{function, general, trait_impls};

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
        try!(write!(w, "impl {} {{", analysis.name));
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

    try!(trait_impls::generate(w, &analysis.name, &analysis.functions, &analysis.specials));

    if generate_trait(analysis) {
        try!(writeln!(w, ""));
        try!(write!(w, "pub trait {}Ext {{", analysis.name));
        for func_analysis in &analysis.methods() {
            try!(function::generate(w, env, func_analysis, true, true, 1));
        }
        try!(writeln!(w, "}}"));

        try!(writeln!(w, ""));
        try!(write!(w, "impl<O: IsA<{}>> {}Ext for O {{", analysis.name, analysis.name));
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
    let mut cfgs: Vec<String> = Vec::new();
    if let Some(cfg) = general::cfg_condition_string(&analysis.cfg_condition, false, 0) {
        cfgs.push(cfg);
    }
    if let Some(cfg) = general::version_condition_string(&env.config.library_name,
                                                         env.config.min_cfg_version,
                                                         analysis.version, false, 0) {
        cfgs.push(cfg);
    }
    contents.push(format!(""));
    contents.extend_from_slice(&cfgs);
    contents.push(format!("mod {};", module_name));
    contents.extend_from_slice(&cfgs);
    contents.push(format!("pub use self::{}::{};", module_name, analysis.name));
    if generate_trait(analysis) {
        contents.extend_from_slice(&cfgs);
        contents.push(format!("pub use self::{}::{}Ext;", module_name, analysis.name));
        for cfg in &cfgs {
            traits.push(format!("\t{}", cfg));
        }
        traits.push(format!("\tpub use super::{}Ext;", analysis.name));
    }
}
