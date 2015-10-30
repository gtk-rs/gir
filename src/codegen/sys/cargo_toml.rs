use std::collections::BTreeMap;
use std::fs::File;
use std::io::prelude::*;
use toml::{self, Array, Parser, Table, Value};

use env::Env;
use file_saver::save_to_file;
use nameutil::crate_name;

pub fn generate(env: &Env) {
    println!("manipulating sys Cargo.toml for {}", env.config.library_name);

    let path = env.config.target_path.join("Cargo.toml");

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

    save_to_file(&path, env.config.make_backup,
        |w| w.write_all(toml::encode_str(&root_table).as_bytes()));
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
        set_string(deps, "bitflags", "0.3");
        set_string(deps, "libc", "0.1");
    }

    {
        let build_deps = upsert_table(root, "build-dependencies");
        set_string(build_deps, "pkg-config", "0.3.5");
        let gcc = upsert_table(build_deps, "gcc");
        set_string(gcc, "version", "0.3.19");
        set_boolean(gcc, "optional", true);
    }

    {
        let features = upsert_table(root, "features");
        let abi_tests = upsert_array(features, "abi_tests");
        if !abi_tests.iter().any(|v| v.as_str() == Some("gcc")) {
            abi_tests.push(Value::String("gcc".into()));
        }
    }
}

fn set_boolean(table: &mut Table, name: &str, new_value: bool) {
    if let Some(v) = table.get_mut(name) {
        *v = Value::Boolean(new_value);
        return;
    }
    table.insert(name.into(), Value::Boolean(new_value));
}

fn set_string<S: Into<String>>(table: &mut Table, name: &str, new_value: S) {
    if let Some(v) = table.get_mut(name) {
        *v = Value::String(new_value.into());
        return;
    }
    table.insert(name.into(), Value::String(new_value.into()));
}

fn upsert_array<S: Into<String>>(parent: &mut Table, name: S) -> &mut Array {
    if let &mut Value::Array(ref mut array) = parent.entry(name.into())
            .or_insert_with(|| Value::Array(Vec::new())) {
        array
    }
    else {
        unreachable!()
    }
}

fn upsert_table<S: Into<String>>(parent: &mut Table, name: S) -> &mut Table {
    if let &mut Value::Table(ref mut table) = parent.entry(name.into())
            .or_insert_with(|| Value::Table(BTreeMap::new())) {
        table
    }
    else {
        unreachable!()
    }
}
