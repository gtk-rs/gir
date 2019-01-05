use analysis::imports::Imports;
use env::Env;
use std::path::Path;
use file_saver;
use codegen::general::{self, cfg_condition, cfg_deprecated, version_condition,
                       version_condition_string};
use library;

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    let path = root_path.join("constants.rs");
    let mut imports = Imports::new(&env.library);

    if env.analysis.constants.is_empty() {
        return;
    }

    imports.add("ffi", None);
    imports.add("std::ffi::CStr", None);

    file_saver::save_to_file(path, env.config.make_backup, |w| {
        general::start_comments(w, &env.config)?;
        general::uses(w, env, &imports)?;
        writeln!(w)?;

        mod_rs.push("\nmod constants;".into());

        for constant in &env.analysis.constants {
            let type_ = env.type_(constant.typ);
            if let library::Type::Fundamental(library::Fundamental::Utf8) = *type_ {
                cfg_deprecated(
                    w,
                    env,
                    constant.deprecated_version,
                    false,
                    0
                )?;
                cfg_condition(w, &constant.cfg_condition, false, 0)?;
                version_condition(w, env, constant.version, false, 0)?;
                writeln!(w, "lazy_static! {{")?;
                writeln!(
                    w,
                    "    pub static ref {name}: &'static str = \
                     unsafe{{CStr::from_ptr(ffi::{c_id}).to_str().unwrap()}};",
                    name = constant.name,
                    c_id = constant.glib_name
                )?;
                writeln!(w, "}}")?;
                if let Some(cfg) = version_condition_string(env, constant.version, false, 0) {
                    mod_rs.push(cfg);
                }
                mod_rs.push(format!("pub use self::constants::{};", constant.name));
            }
        }

        Ok(())
    });
}
