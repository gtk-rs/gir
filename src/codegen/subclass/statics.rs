use std::io::{Result, Write};
use std::fs;

use env::Env;
use config::ExternalLibrary;
use nameutil;
use analysis::namespaces;


// TODO: copied from sys
pub fn generate_extern_crates(w: &mut Write, env: &Env) -> Result<()> {
    for library in &env.config.external_libraries {
        try!(w.write_all(get_extern_crate_string_ffi(library).as_bytes()));
    }
    for library in &env.config.external_libraries {
        try!(w.write_all(get_extern_crate_string(library).as_bytes()));
    }
    for library in &env.config.external_libraries {

        if library.crate_name == "glib"{
            continue;
        }

        let ns = &env.namespaces[namespaces::MAIN];
        if ns.crate_name == library.crate_name {
            continue;
        }

        try!(w.write_all(get_extern_crate_string_subclass(library).as_bytes()));
    }

    Ok(())
}

fn get_extern_crate_string(library: &ExternalLibrary) -> String {

    let mut m = "";
    if library.crate_name == "glib"{
        m = "#[macro_use]\n";
    }

    format!(
        "{}extern crate {};\n",
        m,
        library.crate_name.replace("-", "_")
    )
}
fn get_extern_crate_string_ffi(library: &ExternalLibrary) -> String {
    format!(
        "extern crate {}_sys as {}_ffi;\n",
        library.crate_name.replace("-", "_"),
        nameutil::crate_name(&library.namespace)
    )
}

fn get_extern_crate_string_subclass(library: &ExternalLibrary) -> String {
    format!(
        "#[macro_use]\nextern crate {}_subclass;\n",
        library.crate_name.replace("-", "_")
    )
}


// TODO: copied from sys
pub fn include_custom_modules(w: &mut Write, env: &Env) -> Result<()> {
    let modules = try!(find_modules(env));
    if !modules.is_empty() {
        try!(writeln!(w));
        for module in &modules {
            try!(writeln!(w, "mod {};", module));
        }
        try!(writeln!(w));
        for module in &modules {
            try!(writeln!(w, "pub use {}::*;", module));
        }
    }

    Ok(())
}

// TODO: copied from sys
fn find_modules(env: &Env) -> Result<Vec<String>> {
    let path = env.config.target_path.join("src");

    let mut vec = Vec::<String>::new();
    for entry in try!(fs::read_dir(path)) {
        let path = try!(entry).path();
        let ext = match path.extension() {
            Some(ext) => ext,
            None => continue,
        };
        if ext != "rs" {
            continue;
        }
        let file_stem = path.file_stem().expect("No file name");
        if file_stem == "lib" {
            continue;
        }
        let file_stem = file_stem
            .to_str()
            .expect("Can't convert file name to string")
            .to_owned();
        vec.push(file_stem);
    }
    vec.sort();

    Ok(vec)
}
