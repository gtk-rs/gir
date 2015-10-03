use std::io::prelude::*;
use std::fs::File;
use std::str::FromStr;
use std::error::Error as StdError;
use std::io::Error as IoError;
use std::fmt::{self, Display, Formatter};
use docopt::Docopt;
use docopt::Error as DocoptError;
use toml;

use gobjects;
use version::Version;

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

#[derive(Debug)]
pub enum Error {
    CommandLine(DocoptError),
    Io(IoError, String),
    Toml(String, String),
    Options(String, String),
}

impl StdError for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            CommandLine(ref e) => e.description(),
            Io(ref e, _) => e.description(),
            Toml(ref s, _) => s,
            Options(ref s, _) => s,
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        use self::Error::*;
        match *self {
            CommandLine(ref err) => err.fmt(f),
            Io(ref err, ref filename) => {
                try!(write!(f, "Failed to read config \"{}\": ", filename));
                err.fmt(f)
            }
            Toml(ref err, ref filename) => {
                write!(f, "\"{}\": {}", filename, err)
            }
            Options(ref err, ref filename) => {
                write!(f, "\"{}\": {}", filename, err)
            }
        }
    }
}

impl<'a> From<DocoptError> for Error {
    fn from(e: DocoptError) -> Error {
        Error::CommandLine(e)
    }
}

impl<'a> From<(IoError, &'a str)> for Error {
    fn from(e: (IoError, &'a str)) -> Error {
        Error::Io(e.0, e.1.into())
    }
}

impl<'a, 'b> From<(&'a str, &'b str)> for Error {
    fn from(e: (&'a str, &'b str)) -> Error {
        Error::Options(e.0.into(), e.1.into())
    }
}

impl<'a> From<(String, &'a str)> for Error {
    fn from(e: (String, &'a str)) -> Error {
        Error::Options(e.0, e.1.into())
    }
}

trait LookupStr {
    fn lookup_str<'a>(&'a self, option: &'a str, err: &str, config_file: &str) -> Result<&'a str, Error>;
}

impl LookupStr for toml::Value {
    fn lookup_str<'a>(&'a self, option: &'a str, err: &str, config_file: &str) -> Result<&'a str, Error> {
        let value = try!(self.lookup(option).ok_or((err, config_file)));
        Ok(value.as_str().unwrap())
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
    -b, --make_backup   Make backup before generating
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
    pub min_cfg_version: Version,
    pub make_backup: bool,
}

impl Config {
    pub fn new() -> Result<Config, Error> {
        let args = try!(Docopt::new(USAGE)
            .and_then(|dopt| dopt.parse()));

        let config_file = match args.get_str("-c") {
            "" => "Gir.toml",
            a => a,
        };

        //TODO: add check file existence when stable std::fs::PathExt
        let toml = try!(read_toml(config_file));

        let work_mode_str = match args.get_str("-m") {
            "" => try!(toml.lookup_str("options.work_mode", "No options.work_mode in config", config_file)),
            a => a,
        };
        let work_mode = WorkMode::from_str(work_mode_str)
            .unwrap_or_else(|e| panic!(e));

        let girs_dir = match args.get_str("-d") {
            "" => try!(toml.lookup_str("options.girs_dir", "No options.girs_dir in config", config_file)),
            a => a
        };

        let (library_name, library_version) =
            match (args.get_str("<library>"), args.get_str("<version>")) {
            ("", "") => (
                try!(toml.lookup_str("options.library", "No options.library in config", config_file)),
                try!(toml.lookup_str("options.version", "No options.version in config", config_file))
            ),
            ("", _) | (_, "") => try!(Err(("Library and version can not be specified separately", config_file))),
            (a, b) => (a, b)
        };

        let target_path = match args.get_str("-o") {
            "" => try!(toml.lookup_str("options.target_path", "No target path specified", config_file)),
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

        let min_cfg_version = try!(toml.lookup("options.min_cfg_version")
           .map_or_else(|| Ok(Default::default()), |t| t.as_str().unwrap().parse())
           .map_err(|e: String| (e, config_file)));

        let make_backup = args.get_bool("-b");

        Ok(Config {
            work_mode: work_mode,
            girs_dir: girs_dir.into(),
            library_name: library_name.into(),
            library_version: library_version.into(),
            target_path: target_path.into(),
            external_libraries: external_libraries,
            objects: objects,
            min_cfg_version: min_cfg_version,
            make_backup: make_backup,
        })
    }

    pub fn library_full_name(&self) -> String {
        format!("{}-{}", self.library_name, self.library_version)
    }
}

fn read_toml(filename: &str) -> Result<toml::Value, Error> {
    let mut input = String::new();
    try!(File::open(filename)
         .and_then(|mut f| f.read_to_string(&mut input))
         .map_err(|e| (e, filename)));

    let mut parser = toml::Parser::new(&input);
    match parser.parse() {
        Some(toml) => Ok(toml::Value::Table(toml)),
        None => {
            let err = &parser.errors[parser.errors.len() - 1];
            let (loline, locol) = parser.to_linecol(err.lo);
            let (hiline, hicol) = parser.to_linecol(err.hi);
            let s = format!("{}:{}-{}:{} error: {}", loline, locol, hiline, hicol, err.desc);
            Err(Error::Toml(s, filename.to_owned()))
        }
    }
}
