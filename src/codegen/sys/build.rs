use std::io::{Result, Write};
use std::fs;
use std::path::PathBuf;

use env::Env;
use file_saver::save_to_file;
use library::MAIN_NAMESPACE;
use nameutil::crate_name;
use version::Version;

pub fn generate(env: &Env) {
    println!("generating sys build script for {}", env.config.library_name);

    let path = PathBuf::from(&env.config.target_path)
        .join("build.rs");

    let parent = path.parent().unwrap();
    //TODO: do only if not exists
    let _ = fs::create_dir_all(parent);

    println!("Generating file {:?}", path);
    save_to_file(&path, &mut |w| generate_build_script(w, env));
}

fn generate_build_script<W: Write>(w: &mut W, env: &Env) -> Result<()> {
    try!(writeln!(w, "{}",
r##"
extern crate pkg_config;
use std::cmp::Ordering;
"##));

    let ns = env.library.namespace(MAIN_NAMESPACE);
    try!(writeln!(w, "const LIBRARY_NAME: &'static str = \"{}\";", crate_name(&ns.name)));
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
    println!("cargo:cfg={}", cfgs.connect(" "));
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
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

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.0.cmp(&other.0) {
            Ordering::Equal => {
                match self.1.cmp(&other.1) {
                    Ordering::Equal => self.2.cmp(&other.2),
                    x => x,
                }
            }
            x => x,
        }
    }
}
"##)
}
