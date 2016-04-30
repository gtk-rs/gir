use docopt::Docopt;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml;

use git::repo_hash;
use library::Library;
use super::WorkMode;
use super::gobjects;
use super::error::{Error, TomlHelper};
use version::Version;

static USAGE: &'static str = "
Usage: gir [options] [<library> <version>]
       gir --help

Options:
    -h, --help          Show this message.
    -c CONFIG           Config file path (default: Gir.toml)
    -d GIRSPATH         Directory for girs
    -m MODE             Work mode: doc, normal or sys
    -o PATH             Target path
    -b, --make_backup   Make backup before generating
    -s, --stats         Show statistics
";

#[derive(Debug)]
pub struct Config {
    pub work_mode: WorkMode,
    pub girs_dir: PathBuf,
    pub girs_version: String, //Version in girs_dir, detected by git
    pub library_name: String,
    pub library_version: String,
    pub target_path: PathBuf,
    pub external_libraries: Vec<String>,
    pub objects: gobjects::GObjects,
    pub min_cfg_version: Version,
    pub make_backup: bool,
    pub generate_safety_asserts: bool,
    pub deprecate_by_min_version: bool,
    pub show_statistics: bool,
}

impl Config {
    pub fn new() -> Result<Config, Error> {
        let args = try!(Docopt::new(USAGE)
            .and_then(|dopt| dopt.parse()));

        let config_file: PathBuf = match args.get_str("-c") {
            "" => "Gir.toml",
            a => a,
        }.into();

        let config_dir = match config_file.parent() {
            Some(path) => path.into(),
            None => PathBuf::new(),
        };

        //TODO: add check file existence when stable std::fs::PathExt
        let toml = try!(read_toml(&config_file));

        let work_mode_str = match args.get_str("-m") {
            "" => try!(toml.lookup_str("options.work_mode",
               "No options.work_mode in config", &config_file)),
            a => a,
        };
        let work_mode = WorkMode::from_str(work_mode_str)
            .unwrap_or_else(|e| panic!(e));

        let girs_dir: PathBuf = match args.get_str("-d") {
            "" => {
                let path = try!(toml.lookup_str("options.girs_dir",
                    "No options.girs_dir in config", &config_file));
                config_dir.join(path)
            }
            a => a.into(),
        };
        let girs_version = repo_hash(&girs_dir).unwrap_or_else(|_| "???".into());

        let (library_name, library_version) =
            match (args.get_str("<library>"), args.get_str("<version>")) {
            ("", "") => (
                try!(toml.lookup_str("options.library", "No options.library in config", &config_file)),
                try!(toml.lookup_str("options.version", "No options.version in config", &config_file))
            ),
            ("", _) | (_, "") => try!(Err(Error::options("Library and version can not be specified separately",
                                           &config_file))),
            (a, b) => (a, b)
        };

        let target_path: PathBuf = match args.get_str("-o") {
            "" => {
                let path = try!(toml.lookup_str("options.target_path",
                    "No target path specified", &config_file));
                config_dir.join(path)
            }
            a => a.into()
        };

        let mut objects = toml.lookup("object").map(|t| gobjects::parse_toml(t))
            .unwrap_or_else(|| Default::default());
        gobjects::parse_status_shorthands(&mut objects, &toml);

        let external_libraries = match toml.lookup("options.external_libraries") {
            Some(a) => {
                try!(a.as_result_slice("options.external_libraries", &config_file))
                    .iter().filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }
            None => Vec::new(),
        };

        let min_cfg_version = match toml.lookup("options.min_cfg_version") {
            Some(v) => {
                try!(
                    try!(v.as_result_str("options.min_cfg_version", &config_file))
                        .parse().map_err(|e| Error::options(e, &config_file)))
            }
            None => Default::default(),
        };

        let make_backup = args.get_bool("-b");

        let generate_safety_asserts = match toml.lookup("options.generate_safety_asserts") {
            Some(v) => try!(v.as_result_bool("options.generate_safety_asserts", &config_file)),
            None => false
        };

        let deprecate_by_min_version = match toml.lookup("options.deprecate_by_min_version") {
            Some(v) => try!(v.as_result_bool("options.deprecate_by_min_version", &config_file)),
            None => false
        };

        let show_statistics = args.get_bool("-s");

        Ok(Config {
            work_mode: work_mode,
            girs_dir: girs_dir,
            girs_version: girs_version,
            library_name: library_name.into(),
            library_version: library_version.into(),
            target_path: target_path,
            external_libraries: external_libraries,
            objects: objects,
            min_cfg_version: min_cfg_version,
            make_backup: make_backup,
            generate_safety_asserts: generate_safety_asserts,
            deprecate_by_min_version: deprecate_by_min_version,
            show_statistics: show_statistics,
        })
    }

    pub fn library_full_name(&self) -> String {
        format!("{}-{}", self.library_name, self.library_version)
    }

    pub fn filter_version(&self, version: Option<Version>) -> Option<Version> {
        version.and_then(|v| {
            if v > self.min_cfg_version {
                Some(v)
            } else {
                None
            }
        })
    }

    pub fn resolve_type_ids(&mut self, library: &Library) {
        gobjects::resolve_type_ids(&mut self.objects, library)
    }
}

fn read_toml<P: AsRef<OsStr> + AsRef<Path>>(filename: P) -> Result<toml::Value, Error> {
    let mut input = String::new();
    try!(File::open(&filename)
         .and_then(|mut f| f.read_to_string(&mut input))
         .map_err(|e| Error::io(e, &filename)));

    let mut parser = toml::Parser::new(&input);
    match parser.parse() {
        Some(toml) => Ok(toml::Value::Table(toml)),
        None => {
            let err = &parser.errors[parser.errors.len() - 1];
            let (loline, locol) = parser.to_linecol(err.lo);
            let (hiline, hicol) = parser.to_linecol(err.hi);
            let s = format!("{}:{}-{}:{} error: {}", loline, locol, hiline, hicol, err.desc);
            Err(Error::toml(s, &filename))
        }
    }
}
