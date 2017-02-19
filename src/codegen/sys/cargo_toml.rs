use std::collections::BTreeMap;
use std::fs::File;
use std::io::prelude::*;
use toml::{self, Value};
use toml::value::Table;

use env::Env;
use file_saver::save_to_file;
use nameutil::crate_name;
use version::Version;

pub fn generate(env: &Env) {
    println!("manipulating sys Cargo.toml for {}", env.config.library_name);

    let path = env.config.target_path.join("Cargo.toml");

    let mut toml_str = String::new();
    if let Ok(mut file) = File::open(&path) {
        file.read_to_string(&mut toml_str).unwrap();
    }
    let empty = toml_str.trim().is_empty();
    let mut root_table = toml::from_str(&toml_str).unwrap_or_else(|_| Table::new());

    if empty {
        fill_empty(&mut root_table, env);
    }
    fill_in(&mut root_table, env);

    save_to_file(&path, env.config.make_backup,
        |w| w.write_all(toml::to_string(&root_table).unwrap().as_bytes()));
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
        set_string(dep, "version", "0.2.0");
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
        set_string(deps, "bitflags", "0.4");
        set_string(deps, "libc", "0.2");
    }

    {
        let build_deps = upsert_table(root, "build-dependencies");
        set_string(build_deps, "pkg-config", "0.3.7");
    }

    {
        let features = upsert_table(root, "features");
        features.clear();
        let versions = env.namespaces.main().versions.iter()
            .filter(|&&v| v > env.config.min_cfg_version);
        versions.fold(None::<Version>, |prev, &version| {
            let prev_array: Vec<Value> = prev.iter()
                .map(|v| Value::String(v.to_feature()))
                .collect();
            features.insert(version.to_feature(), Value::Array(prev_array));
            Some(version)
        });
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
