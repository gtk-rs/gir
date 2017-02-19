use std::path::PathBuf;
use toml;

// Create the Error, ErrorKind, ResultExt, and Result types
error_chain! {
    foreign_links {
        CommandLine(::docopt::Error);
        Io(::std::io::Error);
        Log(::log::SetLoggerError);
        Toml(toml::de::Error);
    }

    errors {
        ReadConfig(filename: PathBuf) {
            display("Error read config \"{}\"", filename.display())
        }
        Options(filename: PathBuf) {
            display("Error in config \"{}\"", filename.display())
        }
    }
}

pub trait TomlHelper where Self: Sized {
    fn lookup<'a>(&'a self, option: &str) -> Option<&'a toml::Value>;
    fn lookup_str<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a str>;
    fn lookup_vec<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a Vec<Self>>;
    fn as_result_str<'a>(&'a self, option: &'a str) -> Result<&'a str>;
    fn as_result_vec<'a>(&'a self, option: &'a str) -> Result<&'a Vec<Self>>;
    fn as_result_bool<'a>(&'a self, option: &'a str) -> Result<bool>;
}

impl TomlHelper for toml::Value {
    fn lookup<'a>(&'a self, option: &str) -> Option<&'a toml::Value> {
        let mut value = self;
        for opt in option.split('.') {
            let table = match value.as_table() {
                Some(table) => table,
                None => return None,
            };
            value = match table.get(opt) {
                Some(value) => value,
                None => return None,
            };
        }
        Some(value)
    }
    fn lookup_str<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a str> {
        let value = try!(self.lookup(option).ok_or(err));
        value.as_result_str(option)
    }
    fn lookup_vec<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a Vec<Self>> {
        let value = try!(self.lookup(option).ok_or(err));
        value.as_result_vec(option)
    }
    fn as_result_str<'a>(&'a self, option: &'a str) -> Result<&'a str> {
        self.as_str()
            .ok_or(format!("Invalid `{}` value, expected a string, found {}",
                                          option, self.type_str()).into())
    }
    fn as_result_vec<'a>(&'a self, option: &'a str) -> Result<&'a Vec<Self>> {
        self.as_array()
            .ok_or(format!("Invalid `{}` value, expected a array, found {}",
                           option, self.type_str()).into())
    }
    fn as_result_bool<'a>(&'a self, option: &'a str) -> Result<bool> {
        self.as_bool()
            .ok_or(format!("Invalid `{}` value, expected a boolean, found {}",
                                          option, self.type_str()).into())
    }
}
