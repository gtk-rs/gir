use docopt::Error as DocoptError;
use std::error::Error as StdError;
use std::ffi::OsStr;
use std::fmt::{self, Display, Formatter};
use std::io::Error as IoError;
use std::path::PathBuf;
use toml;

#[derive(Debug)]
pub enum Error {
    CommandLine(DocoptError),
    Io(IoError, PathBuf),
    Toml(String, PathBuf),
    Options(String, PathBuf),
}

impl Error {
    pub fn io<P: AsRef<OsStr>>(error: IoError, filename: P) -> Error {
        Error::Io(error, PathBuf::from(&filename))
    }

    pub fn toml<S: Into<String>, P: AsRef<OsStr>>(error: S, filename: P) -> Error {
        Error::Toml(error.into(), PathBuf::from(&filename))
    }

    pub fn options<S: Into<String>, P: AsRef<OsStr>>(error: S, filename: P) -> Error {
        Error::Options(error.into(), PathBuf::from(&filename))
    }
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
                try!(write!(f, "Failed to read config \"{}\": ", filename.display()));
                err.fmt(f)
            }
            Toml(ref err, ref filename) => {
                write!(f, "\"{}\": {}", filename.display(), err)
            }
            Options(ref err, ref filename) => {
                write!(f, "\"{}\": {}", filename.display(), err)
            }
        }
    }
}

impl From<DocoptError> for Error {
    fn from(e: DocoptError) -> Error {
        Error::CommandLine(e)
    }
}

pub trait TomlHelper where Self: Sized {
    fn lookup_str<'a, P: AsRef<OsStr>>(&'a self, option: &'a str, err: &str, config_file: P) -> Result<&'a str, Error>;
    fn lookup_slice<'a, P: AsRef<OsStr>>(&'a self, option: &'a str, err: &str, config_file: P) -> Result<&'a [Self], Error>;
    fn as_result_str<'a, P: AsRef<OsStr>>(&'a self, option: &'a str, config_file: P) -> Result<&'a str, Error>;
    fn as_result_slice<'a, P: AsRef<OsStr>>(&'a self, option: &'a str, config_file: P) -> Result<&'a [Self], Error>;
    fn as_result_bool<'a, P: AsRef<OsStr>>(&'a self, option: &'a str, config_file: P) -> Result<bool, Error>;
}

impl TomlHelper for toml::Value {
    fn lookup_str<'a, P: AsRef<OsStr>>(&'a self, option: &'a str, err: &str, config_file: P) -> Result<&'a str, Error> {
        let value = try!(self.lookup(option).ok_or(Error::options(err, &config_file)));
        value.as_result_str(option, config_file)
    }
    fn lookup_slice<'a, P: AsRef<OsStr>>(&'a self, option: &'a str, err: &str, config_file: P) -> Result<&'a [Self], Error> {
        let value = try!(self.lookup(option).ok_or(Error::options(err, &config_file)));
        value.as_result_slice(option, config_file)
    }
    fn as_result_str<'a, P: AsRef<OsStr>>(&'a self, option: &'a str, config_file: P) -> Result<&'a str, Error> {
        self.as_str()
            .ok_or(Error::options(format!("Invalid `{}` value, expected a string, found {}",
                                          option, self.type_str()),
                                  config_file))
    }
    fn as_result_slice<'a, P: AsRef<OsStr>>(&'a self, option: &'a str, config_file: P) -> Result<&'a [Self], Error> {
        self.as_slice()
            .ok_or(Error::options(format!("Invalid `{}` value, expected a array, found {}",
                                          option, self.type_str()),
                                  config_file))
    }
    fn as_result_bool<'a, P: AsRef<OsStr>>(&'a self, option: &'a str, config_file: P) -> Result<bool, Error> {
        self.as_bool()
            .ok_or(Error::options(format!("Invalid `{}` value, expected a boolean, found {}",
                                          option, self.type_str()),
                                  config_file))
    }
}
