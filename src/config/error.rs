use std::path::PathBuf;
use toml;

// Create the Error, ErrorKind, ResultExt, and Result types
error_chain! {
    foreign_links {
        CommandLine(::docopt::Error);
        Io(::std::io::Error);
        Log(::log::SetLoggerError);
    }

    errors {
        Toml(error: String, filename: PathBuf) {
            display("\"{}\": {}", filename.display(), error)
        }
        Options(filename: PathBuf) {
            display("Error in config \"{}\"", filename.display())
        }
    }
}

pub trait TomlHelper where Self: Sized {
    fn lookup_str<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a str>;
    fn lookup_slice<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a [Self]>;
    fn as_result_str<'a>(&'a self, option: &'a str) -> Result<&'a str>;
    fn as_result_slice<'a>(&'a self, option: &'a str) -> Result<&'a [Self]>;
    fn as_result_bool<'a>(&'a self, option: &'a str) -> Result<bool>;
}

impl TomlHelper for toml::Value {
    fn lookup_str<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a str> {
        let value = try!(self.lookup(option).ok_or(err));
        value.as_result_str(option)
    }
    fn lookup_slice<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a [Self]> {
        let value = try!(self.lookup(option).ok_or(err));
        value.as_result_slice(option)
    }
    fn as_result_str<'a>(&'a self, option: &'a str) -> Result<&'a str> {
        self.as_str()
            .ok_or(format!("Invalid `{}` value, expected a string, found {}",
                                          option, self.type_str()).into())
    }
    fn as_result_slice<'a>(&'a self, option: &'a str) -> Result<&'a [Self]> {
        self.as_slice()
            .ok_or(format!("Invalid `{}` value, expected a array, found {}",
                                          option, self.type_str()).into())
    }
    fn as_result_bool<'a>(&'a self, option: &'a str) -> Result<bool> {
        self.as_bool()
            .ok_or(format!("Invalid `{}` value, expected a boolean, found {}",
                                          option, self.type_str()).into())
    }
}
