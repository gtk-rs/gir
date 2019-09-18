use crate::{config::Config, env::Env, file_saver::save_to_file, nameutil, version::Version};
use log::info;
use std::{fs::File, io::prelude::*};
use toml::{self, value::Table, Value};

pub fn generate(env: &Env) -> String {
    info!("Generating sys Cargo.toml for {}", env.config.library_name);

    let path = env.config.target_path.join("Cargo.toml");

    let mut toml_str = String::new();
    if let Ok(mut file) = File::open(&path) {
        file.read_to_string(&mut toml_str).unwrap();
    }
    let empty = toml_str.trim().is_empty();
    let mut root_table = toml::from_str(&toml_str).unwrap_or_else(|_| Table::new());
    let crate_name = get_crate_name(&env.config, &root_table);

    if empty {
        fill_empty(&mut root_table, env, &crate_name);
    }
    fill_in(&mut root_table, env);

    save_to_file(&path, env.config.make_backup, |w| {
        w.write_all(toml::to_string(&root_table).unwrap().as_bytes())
    });

    crate_name
}

fn fill_empty(root: &mut Table, env: &Env, crate_name: &str) {
    let package_name = nameutil::exported_crate_name(crate_name);

    {
        let package = upsert_table(root, "package");
        set_string(package, "name", package_name);
        set_string(package, "version", "0.0.1");
        set_string(
            package,
            "links",
            nameutil::crate_name(&env.config.library_name),
        );
    }

    {
        let lib = upsert_table(root, "lib");
        set_string(lib, "name", crate_name);
    }

    let deps = upsert_table(root, "dependencies");
    for ext_lib in &env.config.external_libraries {
        let ext_package = if ext_lib.crate_name == "cairo" {
            format!("{}-sys-rs", ext_lib.crate_name)
        } else if ext_lib.crate_name == "gdk_pixbuf" {
            "gdk-pixbuf-sys".into()
        } else {
            format!("{}-sys", ext_lib.crate_name)
        };
        let dep = upsert_table(deps, &*ext_package);
        if ext_lib.crate_name == "cairo" {
            set_string(dep, "git", "https://github.com/gtk-rs/cairo");
        } else if ext_package.starts_with("sourceview") {
            set_string(dep, "git", "https://github.com/gtk-rs/sourceview");
        } else {
            set_string(dep, "git", "https://github.com/gtk-rs/sys");
        }
    }
}

fn fill_in(root: &mut Table, env: &Env) {
    {
        let package = upsert_table(root, "package");
        set_string(package, "build", "build.rs");
        //set_string(package, "version", "0.2.0");
    }

    {
        let deps = upsert_table(root, "dependencies");
        set_string(deps, "libc", "0.2");
    }

    {
        let build_deps = upsert_table(root, "build-dependencies");
        set_string(build_deps, "pkg-config", "0.3.7");
    }

    {
        let dev_deps = upsert_table(root, "dev-dependencies");
        set_string(dev_deps, "shell-words", "0.1.0");
        set_string(dev_deps, "tempdir", "0.3");
    }

    {
        let features = upsert_table(root, "features");
        let versions = env
            .namespaces
            .main()
            .versions
            .iter()
            .filter(|&&v| v > env.config.min_cfg_version);
        versions.fold(None::<Version>, |prev, &version| {
            let prev_array: Vec<Value> =
                prev.iter().map(|v| Value::String(v.to_feature())).collect();
            features.insert(version.to_feature(), Value::Array(prev_array));
            Some(version)
        });
        features.insert("dox".to_string(), Value::Array(Vec::new()));
    }

    {
        // Small trick to prevent having double quotes around it since toml doesn't like having '.'
        let docs_rs_metadata = upsert_table(root, "package");
        let docs_rs_metadata = upsert_table(docs_rs_metadata, "metadata");
        let docs_rs_metadata = upsert_table(docs_rs_metadata, "docs");
        let docs_rs_metadata = upsert_table(docs_rs_metadata, "rs");
        let mut docs_rs_features = env.config.docs_rs_features.clone();
        docs_rs_features.push("dox".to_owned());
        docs_rs_metadata.insert(
            "features".to_string(),
            Value::Array(
                docs_rs_features
                    .into_iter()
                    .map(Value::String)
                    .collect::<Vec<_>>(),
            ),
        );
    }
}

/// Returns the name of crate being currently generated.
fn get_crate_name(config: &Config, root: &Table) -> String {
    if let Some(&Value::Table(ref lib)) = root.get("lib") {
        if let Some(&Value::String(ref lib_name)) = lib.get("name") {
            //Converting don't needed as library target names cannot contain hyphens
            return lib_name.to_owned();
        }
    }
    if let Some(&Value::Table(ref package)) = root.get("package") {
        if let Some(&Value::String(ref package_name)) = package.get("name") {
            return nameutil::crate_name(package_name);
        }
    }
    return format!("{}_sys", nameutil::crate_name(&config.library_name));
}

fn set_string<S: Into<String>>(table: &mut Table, name: &str, new_value: S) {
    if let Some(v) = table.get_mut(name) {
        *v = Value::String(new_value.into());
        return;
    }
    table.insert(name.into(), Value::String(new_value.into()));
}

fn upsert_table<S: Into<String>>(parent: &mut Table, name: S) -> &mut Table {
    if let Value::Table(ref mut table) = *parent
        .entry(name.into())
        .or_insert_with(|| Value::Table(toml::map::Map::new()))
    {
        table
    } else {
        unreachable!()
    }
}
