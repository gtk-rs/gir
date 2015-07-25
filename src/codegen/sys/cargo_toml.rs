use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::prelude::*;
use std::path::PathBuf;
use toml::{self, Parser, Table, Value};

use env::Env;
use file_saver::save_to_file;
use nameutil::crate_name;

pub fn generate(env: &Env) {
    println!("manipulating sys Cargo.toml for {}", env.config.library_name);

    let path = PathBuf::from(&env.config.target_path)
        .join("Cargo.toml");

    let parent = path.parent().unwrap();
    //TODO: do only if not exists
    let _ = fs::create_dir(parent);

    let mut toml_str = String::new();
    if let Ok(mut file) = File::open(&path) {
        file.read_to_string(&mut toml_str).unwrap();
    }
    let empty = toml_str.trim().is_empty();
    let mut parser = Parser::new(&toml_str);
    let mut root_table = parser.parse().unwrap_or_else(BTreeMap::new);

    if empty {
        fill_empty(&mut root_table, env);
    }
    fill_in(&mut root_table, env);
    
    save_to_file(&path, &mut |w| w.write_all(toml::encode_str(&root_table).as_bytes()));
}

fn fill_empty(root: &mut Table, env: &Env) {
    let name = format!("{}_sys", crate_name(&env.config.library_name));
    let package_name = name.replace("_", "-");

    {
        let package = upsert_table(root, "package");
        set_string(package, "name", package_name);
        set_string(package, "version", "0.2.0");
    }

    {
        let lib = upsert_table(root, "lib");
        set_string(lib, "name", name);
    }

    let deps = upsert_table(root, "dependencies");
    for ext_lib in &env.config.external_libraries {
        let ext_package = format!("{}_sys", crate_name(ext_lib))
            .replace("_", "-");
        let dep = upsert_table(deps, &*ext_package);
        set_string(dep, "path", format!("../{}", ext_package));
        set_string(dep, "version", "^0.2.0");
    }
}

fn fill_in(root: &mut Table, env: &Env) {
    {
        let package = upsert_table(root, "package");
        set_string(package, "build", "build.rs");
        set_string(package, "links", crate_name(&env.config.library_name));
        //set_string(package, "version", "0.2.0");
    }

    {
        let deps = upsert_table(root, "dependencies");
        set_string(deps, "bitflags", "^0.3");
        set_string(deps, "libc", "^0.1");
    }

    {
        let build_deps = upsert_table(root, "build-dependencies");
        set_string(build_deps, "pkg-config", "^0.3.5");
    }
}

fn set_string<S: Into<String>>(table: &mut Table, name: &str, new_value: S) {
    if let Some(v) = table.get_mut(name) {
        *v = Value::String(new_value.into());
        return;
    }
    table.insert(name.into(), Value::String(new_value.into()));
}

fn upsert_table<'a, S: Into<String>>(parent: &'a mut Table, name: S) -> &'a mut Table {
    if let &mut Value::Table(ref mut table) = parent.entry(name.into())
            .or_insert_with(|| Value::Table(BTreeMap::new())) {
        table
    }
    else {
        unreachable!()
    }
}
