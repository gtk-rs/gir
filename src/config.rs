use std::io::prelude::*;
use std::fs::File;
use std::str::FromStr;
use docopt::Docopt;
use toml;

use gobjects;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkMode {
    Normal,     //generate widgets etc.
    Sys,        //generate -sys with ffi
}

impl Default for WorkMode {
    fn default() -> WorkMode { WorkMode::Normal }
}

impl FromStr for WorkMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "normal" => Ok(WorkMode::Normal),
            "sys" => Ok(WorkMode::Sys),
            _ => Err("Wrong work mode".into())
        }
    }
}

static USAGE: &'static str = "
Usage: gir [options] [<library> <version>]
       gir --help

Options:
    -h, --help          Show this message.
    -c CONFIG           Config file path (default: Gir.toml)
    -d GIRSPATH         Directory for girs
    -m MODE             Work mode: normal or sys
    -o PATH             Target path
";

#[derive(Debug)]
pub struct Config {
    pub work_mode: WorkMode,
    pub girs_dir: String,
    pub library_name: String,
    pub library_version: String,
    pub target_path: String,
    pub external_libraries: Vec<String>,
    pub objects: gobjects::GObjects,
}

impl Config {
    pub fn new() -> Config {
        let args = Docopt::new(USAGE)
            .and_then(|dopt| dopt.parse())
            .unwrap_or_else(|e| e.exit());

        let config_file = match args.get_str("-c") {
            "" => "Gir.toml",
            a => a,
        };

        //TODO: add check file existence when stable std::fs::PathExt
        let toml = read_toml(config_file);

        let work_mode_str = match args.get_str("-m") {
            "" => toml.lookup("options.work_mode")
                    .expect("No options.work_mode in config")
                    .as_str().unwrap(),
            a => a,
        };
        let work_mode = WorkMode::from_str(work_mode_str)
            .unwrap_or_else(|e| panic!(e));

        let girs_dir = match args.get_str("-d") {
            "" => toml.lookup("options.girs_dir")
                      .expect("No options.girs_dir in config")
                      .as_str().unwrap(),
            a => a
        };

        let (library_name, library_version) =
            match (args.get_str("<library>"), args.get_str("<version>")) {
            ("", "") => (
                toml.lookup("options.library")
                    .expect("No options.library in config")
                    .as_str().unwrap(),
                toml.lookup("options.version")
                    .expect("No options.version in config")
                    .as_str().unwrap()
            ),
            ("", _) | (_, "") => panic!("Library and version can not be specified separately"),
            (a, b) => (a, b)
        };

        let target_path = match args.get_str("-o") {
            "" => toml.lookup("options.target_path")
                    .expect("No target path specified")
                    .as_str().unwrap(),
            a => a
        };

        let mut objects = toml.lookup("object").map(|t| gobjects::parse_toml(t))
            .unwrap_or_else(|| Default::default());
        gobjects::parse_status_shorthands(&mut objects, &toml);

        let external_libraries = toml.lookup("options.external_libraries")
            .map(|a| a.as_slice().unwrap().iter()
                .filter_map(|v|
                    if let &toml::Value::String(ref s) = v { Some(s.clone()) } else { None } )
                .collect())
            .unwrap_or_else(|| Vec::new());

        Config {
            work_mode: work_mode,
            girs_dir: girs_dir.into(),
            library_name: library_name.into(),
            library_version: library_version.into(),
            target_path: target_path.into(),
            external_libraries: external_libraries,
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
