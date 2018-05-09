use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StringType {
    String,        // &str for input, String for return
    Filename,      // Path for input, PathBuf for return
    OsString,      // OsStr for input, OsString for return
}

impl FromStr for StringType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "string" => Ok(StringType::String),
            "filename" => Ok(StringType::Filename),
            "os_string" => Ok(StringType::OsString),
            _ => Err("Wrong string type".into()),
        }
    }
}
