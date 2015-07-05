use std::io::prelude::*;
use std::fs::File;
use docopt::Docopt;
use toml;

use gobjects;

static USAGE: &'static str = "
Usage: gir [options] [<library> <version>]

Options:
    -d PATH             Directory for girs
    -o PATH             Target root path
    -s NAME             Special type name without prefix
";

#[derive(Debug)]
pub struct Config {
    pub girs_dir: String,
    pub library_name: String,
    pub library_version: String,
    pub target_path: String,
    pub special_type: String,
    pub objects: gobjects::GObjects,
}

impl Config {
    pub fn new() -> Config {
        let args = Docopt::new(USAGE)
            .and_then(|dopt| dopt.parse())
            .unwrap_or_else(|e| e.exit());

        let toml = read_toml("Gir.toml");

        let girs_dir = match args.get_str("-d") {
            "" => toml.lookup("options.girs_dir")
                      .unwrap_or_else(|| panic!("No options.girs_dir in config"))
                      .as_str().unwrap(),
            a => a
        };

        let (library_name, library_version) =
            match (args.get_str("<library>"), args.get_str("<version>")) {
            ("", "") => (
                toml.lookup("options.library")
                    .unwrap_or_else(|| panic!("No options.library in config"))
                    .as_str().unwrap(),
                toml.lookup("options.version")
                    .unwrap_or_else(|| panic!("No options.version in config"))
                    .as_str().unwrap()
            ),
            ("", _) | (_, "") => panic!("Library and version can not be specified separately"),
            (a, b) => (a, b)
        };

        let target_path = match args.get_str("-o") {
            "" => toml.lookup("options.target_path")
                    .unwrap_or_else(|| panic!("No options.target_path in config"))
                    .as_str().unwrap(),
            a => a
        };

        let special_type = match args.get_str("-s") {
            "" => toml.lookup("options.special_type")
                    .unwrap_or_else(|| panic!("No options.special_type in config"))
                    .as_str().unwrap(),
            a => a
        };

        let objects = gobjects::parse_toml(toml.lookup("object").unwrap());

        Config {
            girs_dir: girs_dir.into(),
            library_name: library_name.into(),
            library_version: library_version.into(),
            target_path: target_path.into(),
            special_type: special_type.into(),
            objects: objects,
        }
    }

    pub fn library_full_name(&self) -> String {
        format!("{}-{}", self.library_name, self.library_version)
    }
}

fn read_toml(filename: &str) -> toml::Value {
    let mut input = String::new();
    File::open(filename).and_then(|mut f| {
        f.read_to_string(&mut input)
    }).unwrap();
    let mut parser = toml::Parser::new(&input);
    match parser.parse() {
        Some(toml) => toml::Value::Table(toml),
        None => {
            for err in &parser.errors {
                let (loline, locol) = parser.to_linecol(err.lo);
                let (hiline, hicol) = parser.to_linecol(err.hi);
                println!("{}:{}:{}-{}:{} error: {}",
                         filename, loline, locol, hiline, hicol, err.desc);
            }
            panic!("Errors in config")
        }
    }
}
