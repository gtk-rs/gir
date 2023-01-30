use std::{
    fmt::Display,
    io::{Result, Write},
    path::Path,
};

use general::{cfg_condition, version_condition};

use crate::{
    config::{gobjects::GObject, WorkMode},
    env::Env,
    file_saver::*,
    library::Member,
    version::Version,
};

mod alias;
mod bound;
mod child_properties;
mod constants;
mod doc;
mod enums;
mod flags;
pub mod function;
mod function_body_chunk;
mod functions;
mod general;
mod object;
mod objects;
mod parameter;
mod properties;
mod property_body;
mod record;
mod records;
mod ref_mode;
mod return_value;
mod signal;
mod signal_body;
mod special_functions;
mod sys;
mod trait_impls;
mod trampoline;
mod trampoline_from_glib;
mod visibility;
pub use visibility::Visibility;
mod trampoline_to_glib;
pub mod translate_from_glib;
pub mod translate_to_glib;

pub fn generate(env: &Env) {
    match env.config.work_mode {
        WorkMode::Normal => normal_generate(env),
        WorkMode::Sys => sys::generate(env),
        WorkMode::Doc => doc::generate(env),
        WorkMode::DisplayNotBound => {}
    }
}

fn normal_generate(env: &Env) {
    let mut mod_rs: Vec<String> = Vec::new();
    let mut traits: Vec<String> = Vec::new();
    let mut builders: Vec<String> = Vec::new();
    let root_path = env.config.auto_path.as_path();

    generate_single_version_file(env);
    objects::generate(env, root_path, &mut mod_rs, &mut traits, &mut builders);
    records::generate(env, root_path, &mut mod_rs);
    enums::generate(env, root_path, &mut mod_rs);
    flags::generate(env, root_path, &mut mod_rs);
    alias::generate(env, root_path, &mut mod_rs);
    functions::generate(env, root_path, &mut mod_rs);
    constants::generate(env, root_path, &mut mod_rs);

    generate_mod_rs(env, root_path, &mod_rs, &traits, &builders);
}

pub fn generate_mod_rs(
    env: &Env,
    root_path: &Path,
    mod_rs: &[String],
    traits: &[String],
    builders: &[String],
) {
    let path = root_path.join("mod.rs");
    save_to_file(path, env.config.make_backup, |w| {
        general::start_comments(w, &env.config)?;
        general::write_vec(w, mod_rs)?;
        writeln!(w)?;
        if !traits.is_empty() {
            writeln!(w, "#[doc(hidden)]")?;
            writeln!(w, "pub mod traits {{")?;
            general::write_vec(w, traits)?;
            writeln!(w, "}}")?;
        }

        if !builders.is_empty() {
            writeln!(w, "#[doc(hidden)]")?;
            writeln!(w, "pub mod builders {{")?;
            general::write_vec(w, builders)?;
            writeln!(w, "}}")?;
        }
        Ok(())
    });
}

pub fn generate_single_version_file(env: &Env) {
    if let Some(ref path) = env.config.single_version_file {
        save_to_file(path, env.config.make_backup, |w| {
            general::single_version_file(w, &env.config, "")
        });
    }
}

pub fn generate_default_impl<
    'a,
    D: Display,
    F: Fn(&'a Member) -> Option<(Option<Version>, Option<&'a String>, D)>,
>(
    w: &mut dyn Write,
    env: &Env,
    config: &GObject,
    type_name: &str,
    type_version: Option<Version>,
    mut members: impl Iterator<Item = &'a Member>,
    callback: F,
) -> Result<()> {
    if let Some(ref default_value) = config.default_value {
        let member = match members.find(|m| m.name == *default_value) {
            Some(m) => m,
            None => {
                log::error!(
                    "type `{}` doesn't have a member named `{}`. Not generating default impl.",
                    type_name,
                    default_value,
                );
                return Ok(());
            }
        };
        let (version, cfg_cond, member_name) = match callback(member) {
            Some(n) => n,
            None => {
                log::error!(
                    "member `{}` on type `{}` isn't generated so no default impl.",
                    default_value,
                    type_name,
                );
                return Ok(());
            }
        };

        // First we generate the type cfg.
        version_condition(w, env, None, type_version, false, 0)?;
        cfg_condition(w, config.cfg_condition.as_ref(), false, 0)?;
        // Then we generate the member cfg.
        version_condition(w, env, None, version, false, 0)?;
        cfg_condition(w, cfg_cond, false, 0)?;
        writeln!(
            w,
            "\n\
             impl Default for {type_name} {{\n\
             \tfn default() -> Self {{\n\
             \t\tSelf::{member_name}\n\
             \t}}\n\
             }}\n",
        )
    } else {
        Ok(())
    }
}
