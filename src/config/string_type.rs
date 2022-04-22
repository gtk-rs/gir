use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StringType {
    Utf8,     // &str for input, String for return
    Filename, // Path for input, PathBuf for return
    OsString, // OsStr for input, OsString for return
}

impl FromStr for StringType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "utf8" => Ok(StringType::Utf8),
            "filename" => Ok(StringType::Filename),
            "os_string" => Ok(StringType::OsString),
            _ => Err(format!("Wrong string type '{}'", s)),
        }
    }
}
