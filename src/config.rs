use std::io::prelude::*;
use std::fs::File;
use docopt::Docopt;
use toml;

use gobjects;

static USAGE: &'static str = "
Usage: gir [-d <girs_dir>] [<library>]

Options:
    -d PATH            Directory for girs
";

#[derive(Debug)]
pub struct Config {
    pub girs_dir: String,
    pub library_name: String,
    pub objects: gobjects::GObjects,
}

impl Config {
    pub fn new() -> Config {
        let args = Docopt::new(USAGE).unwrap()
            .parse().unwrap_or_else(|e| e.exit());

        let toml = read_toml("Gir.toml");

        let girs_dir = match args.get_str("-d") {
            "" => toml.lookup("options.girs_dir")
                      .unwrap_or_else(|| panic!("No options.girs_dir in config"))
                      .as_str().unwrap(),
            a => a
        };

        let library_name = match args.get_str("<library>") {
            "" => toml.lookup("options.library")
                    .unwrap_or_else(|| panic!("No options.library in config"))
                    .as_str().unwrap(),
            a => a
        };

        let objects = gobjects::parse_toml(toml.lookup("object").unwrap());

        Config {
            girs_dir: girs_dir.to_string(),
            library_name: library_name.to_string(),
            objects: objects,
        }
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
