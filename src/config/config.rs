use docopt::{self, Docopt};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use toml;

use git::repo_hash;
use library;
use library::Library;
use super::external_libraries::{read_external_libraries, ExternalLibrary};
use super::WorkMode;
use super::gobjects;
use super::error::*;
use version::Version;

static USAGE: &'static str = "
Usage: gir [options] [<library> <version>]
       gir --help

Options:
    -h, --help              Show this message.
    -c CONFIG               Config file path (default: Gir.toml)
    -d GIRSPATH             Directory for girs
    -m MODE                 Work mode: doc, normal, sys or not_bound
    -o PATH                 Target path
    --doc-target-path PATH  Doc target path
    -b, --make_backup       Make backup before generating
    -s, --stats             Show statistics
";

#[derive(Debug)]
pub struct Config {
    pub work_mode: WorkMode,
    pub girs_dir: PathBuf,
    pub girs_version: String, //Version in girs_dir, detected by git
    pub library_name: String,
    pub library_version: String,
    pub target_path: PathBuf,
    pub doc_target_path: PathBuf,
    pub external_libraries: Vec<ExternalLibrary>,
    pub objects: gobjects::GObjects,
    pub min_cfg_version: Version,
    pub make_backup: bool,
    pub generate_safety_asserts: bool,
    pub deprecate_by_min_version: bool,
    pub show_statistics: bool,
    pub concurrency: library::Concurrency,
}

impl Config {
    pub fn new() -> Result<Config> {
        let args = try!(Docopt::new(USAGE).and_then(|dopt| dopt.parse()));

        let config_file: PathBuf = match args.get_str("-c") {
            "" => "Gir.toml",
            a => a,
        }.into();

        let config_dir = match config_file.parent() {
            Some(path) => path.into(),
            None => PathBuf::new(),
        };

        let toml =
            try!(read_toml(&config_file).chain_err(|| ErrorKind::ReadConfig(config_file.clone())));

        Config::process_options(&args, &toml, &config_dir)
            .chain_err(|| ErrorKind::Options(config_file))
    }

    fn process_options(
        args: &docopt::ArgvMap,
        toml: &toml::Value,
        config_dir: &Path,
    ) -> Result<Config> {
        let work_mode_str = match args.get_str("-m") {
            "" => try!(toml.lookup_str("options.work_mode", "No options.work_mode")),
            a => a,
        };
        let work_mode = try!(WorkMode::from_str(work_mode_str));

        let girs_dir: PathBuf = match args.get_str("-d") {
            "" => {
                let path = try!(toml.lookup_str("options.girs_dir", "No options.girs_dir"));
                config_dir.join(path)
            }
            a => a.into(),
        };
        let girs_version = repo_hash(&girs_dir).unwrap_or_else(|_| "???".into());

        let (library_name, library_version) =
            match (args.get_str("<library>"), args.get_str("<version>")) {
                ("", "") => (
                    try!(toml.lookup_str("options.library", "No options.library")),
                    try!(toml.lookup_str("options.version", "No options.version")),
                ),
                ("", _) | (_, "") => bail!("Library and version can not be specified separately"),
                (a, b) => (a, b),
            };

        let target_path: PathBuf = match args.get_str("-o") {
            "" => {
                let path = try!(toml.lookup_str(
                    "options.target_path",
                    "No target path specified",
                ));
                config_dir.join(path)
            }
            a => a.into(),
        };

        let doc_target_path: PathBuf = match args.get_str("--doc-target-path") {
            "" => {
                match toml.lookup("options.doc_target_path") {
                    Some(p) => config_dir.join(try!(p.as_result_str("options.doc_target_path"))),
                    None => target_path.join("vendor.md"),
                }
            },
            p => config_dir.join(p),
        };

        let concurrency = match toml.lookup("options.concurrency") {
            Some(v) => try!(try!(v.as_result_str("options.concurrency")).parse()),
            None => Default::default(),
        };

        // options.concurrency is the default of all objects if nothing
        // else is configured
        let mut objects = toml.lookup("object")
            .map(|t| gobjects::parse_toml(t, concurrency))
            .unwrap_or_default();
        gobjects::parse_status_shorthands(&mut objects, toml, concurrency);

        let external_libraries = try!(read_external_libraries(toml));

        let min_cfg_version = match toml.lookup("options.min_cfg_version") {
            Some(v) => try!(try!(v.as_result_str("options.min_cfg_version")).parse()),
            None => Default::default(),
        };

        let make_backup = args.get_bool("-b");

        let generate_safety_asserts = match toml.lookup("options.generate_safety_asserts") {
            Some(v) => try!(v.as_result_bool("options.generate_safety_asserts")),
            None => false,
        };

        let deprecate_by_min_version = match toml.lookup("options.deprecate_by_min_version") {
            Some(v) => try!(v.as_result_bool("options.deprecate_by_min_version")),
            None => false,
        };

        let show_statistics = args.get_bool("-s");

        Ok(Config {
            work_mode: work_mode,
            girs_dir: girs_dir,
            girs_version: girs_version,
            library_name: library_name.into(),
            library_version: library_version.into(),
            target_path: target_path,
            doc_target_path: doc_target_path,
            external_libraries: external_libraries,
            objects: objects,
            min_cfg_version: min_cfg_version,
            make_backup: make_backup,
            generate_safety_asserts: generate_safety_asserts,
            deprecate_by_min_version: deprecate_by_min_version,
            show_statistics: show_statistics,
            concurrency: concurrency,
        })
    }

    pub fn library_full_name(&self) -> String {
        format!("{}-{}", self.library_name, self.library_version)
    }

    pub fn filter_version(&self, version: Option<Version>) -> Option<Version> {
        version.and_then(|v| if v > self.min_cfg_version {
            Some(v)
        } else {
            None
        })
    }

    pub fn resolve_type_ids(&mut self, library: &Library) {
        gobjects::resolve_type_ids(&mut self.objects, library)
    }
}

fn read_toml<P: AsRef<Path>>(filename: P) -> Result<toml::Value> {
    if !filename.as_ref().is_file() {
        bail!("Config don't exists or not file");
    }
    let mut input = String::new();
    try!(File::open(&filename).and_then(|mut f| f.read_to_string(&mut input)));

    let toml = try!(toml::from_str(&input));

    Ok(toml)
}
