use std::io::{Result, Write};

use env::Env;
use file_saver::save_to_file;
use regex::Regex;

pub fn generate(env: &Env) {
    println!("generating sys build script for {}", env.config.library_name);

    let path = env.config.target_path.join("build.rs");

    println!("Generating file {:?}", path);
    save_to_file(&path, env.config.make_backup,
        |w| generate_build_script(w, env));
}

fn generate_build_script(w: &mut Write, env: &Env) -> Result<()> {
    try!(write!(w, "{}",
r##"extern crate pkg_config;

use pkg_config::{Config, Error};
use std::env;
use std::io::prelude::*;
use std::io;
use std::process;

fn main() {
    if let Err(s) = find() {
        let _ = writeln!(io::stderr(), "{}", s);
        process::exit(1);
    }
}

fn find() -> Result<(), Error> {
"##));

    let ns = env.namespaces.main();
    let regex = Regex::new(r"^lib(.+)\.so.*$").expect("Regex failed");
    let shared_libs: Vec<_> = ns.shared_libs.iter()
        .map(|s| regex.replace(s, "\"$1\""))
        .collect();

    try!(writeln!(w, "\tlet package_name = \"{}\";",
                  ns.package_name.as_ref().expect("Package name doesn't exist")));
    try!(writeln!(w, "\tlet shared_libs = [{}];", shared_libs.join(", ")));
    try!(write!(w, "\tlet version = "));
    let versions = ns.versions.iter()
        .filter(|v| **v >= env.config.min_cfg_version)
        .skip(1)
        .collect::<Vec<_>>();
    for v in versions.iter().rev() {
        try!(write!(w, "if cfg!({}) {{\n\t\t\"{}\"\n\t}} else ", v.to_cfg(), v));
    }
    try!(writeln!(w, "{{\n\t\t\"{}\"\n\t}};", env.config.min_cfg_version));

    writeln!(w, "{}",
r##"
    if let Ok(lib_dir) = env::var("GTK_LIB_DIR") {
        for lib_ in shared_libs.iter() {
            println!("cargo:rustc-link-lib=dylib={}", lib_);
        }
        println!("cargo:rustc-link-search=native={}", lib_dir);
        return Ok(())
    }

    let target = env::var("TARGET").expect("TARGET environment variable doesn't exist");
    let hardcode_shared_libs = target.contains("windows");

    let mut config = Config::new();
    config.atleast_version(version);
    if hardcode_shared_libs {
        config.cargo_metadata(false);
    }
    match config.probe(package_name) {
        Ok(library) => {
            if hardcode_shared_libs {
                for lib_ in shared_libs.iter() {
                    println!("cargo:rustc-link-lib=dylib={}", lib_);
                }
                for path in library.link_paths.iter() {
                    println!("cargo:rustc-link-search=native={}",
                             path.to_str().expect("library path doesn't exist"));
                }
            }
            Ok(())
        }
        Err(Error::EnvNoPkgConfig(_)) | Err(Error::Command { .. }) => {
            for lib_ in shared_libs.iter() {
                println!("cargo:rustc-link-lib=dylib={}", lib_);
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}
"##)
}
