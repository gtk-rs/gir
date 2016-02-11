use std::io::{Result, Write};

use env::Env;
use file_saver::save_to_file;
use version::Version;

pub fn generate(env: &Env) {
    println!("generating sys build script for {}", env.config.library_name);

    let path = env.config.target_path.join("build.rs");

    println!("Generating file {:?}", path);
    save_to_file(&path, env.config.make_backup,
        |w| generate_build_script(w, env));
}

fn generate_build_script(w: &mut Write, env: &Env) -> Result<()> {
    try!(writeln!(w, "{}", "extern crate pkg_config;\n"));

    let ns = env.namespaces.main();
    try!(writeln!(w, "const LIBRARY_NAME: &'static str = \"{}\";", ns.crate_name));
    try!(writeln!(w, "const PACKAGE_NAME: &'static str = \"{}\";",
                  ns.package_name.as_ref().unwrap()));
    try!(writeln!(w, "const VERSIONS: &'static [Version] = &["));
    let versions = ns.versions.iter()
        .filter(|v| **v >= env.config.min_cfg_version);
    for &Version(major, minor, patch) in versions {
        try!(writeln!(w, "\tVersion({}, {}, {}),", major, minor, patch));
    }
    try!(writeln!(w, "];"));

    writeln!(w, "{}",
r##"
fn main() {
    let lib = pkg_config::find_library(PACKAGE_NAME)
        .unwrap_or_else(|e| panic!("{}", e));
    let version = Version::new(&lib.version);
    let mut cfgs = Vec::new();
    for v in VERSIONS.iter().filter(|&&v| v <= version) {
        let cfg = v.to_cfg();
        println!("cargo:rustc-cfg={}", cfg);
        cfgs.push(cfg);
    }
    println!("cargo:cfg={}", cfgs.join(" "));
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct Version(pub u16, pub u16, pub u16);

impl Version {
    fn new(s: &str) -> Version {
        let mut parts = s.splitn(4, '.')
            .map(|s| s.parse())
            .take_while(Result::is_ok)
            .map(Result::unwrap);
        Version(parts.next().unwrap_or(0),
            parts.next().unwrap_or(0), parts.next().unwrap_or(0))
    }

    fn to_cfg(&self) -> String {
        match *self {
            Version(major, minor, 0) => format!("{}_{}_{}", LIBRARY_NAME, major, minor),
            Version(major, minor, patch) =>
                format!("{}_{}_{}_{}", LIBRARY_NAME, major, minor, patch),
        }
    }
}
"##)
}
