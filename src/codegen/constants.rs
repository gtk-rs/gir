use std::path::Path;

use crate::{
    analysis::imports::Imports,
    codegen::general::{
        self, cfg_condition, cfg_deprecated, doc_alias, version_condition, version_condition_string,
    },
    env::Env,
    file_saver, library,
};

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    let path = root_path.join("constants.rs");
    let mut imports = Imports::new(&env.library);

    if env.analysis.constants.is_empty() {
        return;
    }

    let sys_crate_name = env.main_sys_crate_name();
    imports.add("glib::GStr");

    file_saver::save_to_file(path, env.config.make_backup, |w| {
        general::start_comments(w, &env.config)?;
        general::uses(w, env, &imports, None)?;
        writeln!(w)?;

        mod_rs.push("\nmod constants;".into());

        for constant in &env.analysis.constants {
            let type_ = env.type_(constant.typ);
            if let library::Type::Basic(library::Basic::Utf8) = type_ {
                cfg_deprecated(w, env, None, constant.deprecated_version, false, 0)?;
                cfg_condition(w, constant.cfg_condition.as_ref(), false, 0)?;
                version_condition(w, env, None, constant.version, false, 0)?;
                doc_alias(w, &constant.glib_name, "", 0)?;
                writeln!(
                    w,
                    "pub static {name}: &GStr = unsafe{{GStr::from_utf8_with_nul_unchecked({sys_crate_name}::{c_id})}};",
                    sys_crate_name = sys_crate_name,
                    name = constant.name,
                    c_id = constant.glib_name
                )?;
                if let Some(cfg) = version_condition_string(env, None, constant.version, false, 0) {
                    mod_rs.push(cfg);
                }
                mod_rs.push(format!(
                    "{}pub use self::constants::{};",
                    constant
                        .deprecated_version
                        .map(|_| "#[allow(deprecated)]\n")
                        .unwrap_or(""),
                    constant.name
                ));
            }
        }

        Ok(())
    });
}
