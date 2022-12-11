use std::{
    io::{prelude::*, Result},
    path::Path,
};

use crate::{
    analysis::{namespaces, rust_type::RustType},
    codegen::general,
    config::gobjects::GObject,
    env::Env,
    file_saver,
    library::*,
    traits::*,
};

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    let configs: Vec<&GObject> = env
        .config
        .objects
        .values()
        .filter(|c| {
            c.status.need_generate() && c.type_id.map_or(false, |tid| tid.ns_id == namespaces::MAIN)
        })
        .collect();
    let mut has_any = false;
    for config in &configs {
        if let Type::Alias(_) = env.library.type_(config.type_id.unwrap()) {
            has_any = true;
            break;
        }
    }

    if !has_any {
        return;
    }

    let path = root_path.join("alias.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        general::start_comments(w, &env.config)?;
        writeln!(w)?;
        writeln!(w, "#[allow(unused_imports)]")?;
        writeln!(w, "use crate::auto::*;")?;
        writeln!(w)?;

        mod_rs.push("\nmod alias;".into());
        for config in &configs {
            if let Type::Alias(alias) = env.library.type_(config.type_id.unwrap()) {
                mod_rs.push(format!("pub use self::alias::{};", alias.name));
                generate_alias(env, w, alias, config)?;
            }
        }

        Ok(())
    });
}

fn generate_alias(env: &Env, w: &mut dyn Write, alias: &Alias, _: &GObject) -> Result<()> {
    let typ = RustType::try_new(env, alias.typ).into_string();
    writeln!(w, "pub type {} = {};", alias.name, typ)?;

    Ok(())
}
