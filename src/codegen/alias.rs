use analysis::namespaces;
use analysis::rust_type::rust_type;
use codegen::general;
use config::gobjects::GObject;
use env::Env;
use file_saver;
use library::*;
use std::io::prelude::*;
use std::io::Result;
use std::path::Path;
use traits::*;

pub fn generate(env: &Env, root_path: &Path, mod_rs: &mut Vec<String>) {
    let configs: Vec<&GObject> = env.config
        .objects
        .values()
        .filter(|c| {
            c.status.need_generate() && c.type_id.map_or(false, |tid| tid.ns_id == namespaces::MAIN)
        })
        .collect();
    let mut has_any = false;
    for config in &configs {
        if let Type::Alias(_) = *env.library.type_(config.type_id.unwrap()) {
            has_any = true;
            break;
        }
    }

    if !has_any {
        return;
    }

    let path = root_path.join("alias.rs");
    file_saver::save_to_file(path, env.config.make_backup, |w| {
        try!(general::start_comments(w, &env.config));
        try!(writeln!(w, ""));
        try!(writeln!(w, "#[allow(unused_imports)]"));
        try!(writeln!(w, "use auto::*;"));
        try!(writeln!(w, ""));

        mod_rs.push("\nmod alias;".into());
        for config in &configs {
            if let Type::Alias(ref alias) = *env.library.type_(config.type_id.unwrap()) {
                mod_rs.push(format!("pub use self::alias::{};", alias.name));
                try!(generate_alias(env, w, alias, config));
            }
        }

        Ok(())
    });
}

fn generate_alias(env: &Env, w: &mut Write, alias: &Alias, _: &GObject) -> Result<()> {
    let typ = rust_type(env, alias.typ).into_string();
    try!(writeln!(w, "pub type {} = {};", alias.name, typ));

    Ok(())
}
