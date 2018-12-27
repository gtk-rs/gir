use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkMode {
    Normal,          // generate widgets etc.
    Sys,             // generate -sys with ffi
    Doc,             // generate documentation file
    Subclass,         // generate subclassing traits
    DisplayNotBound, // Show not bound types
}

impl Default for WorkMode {
    fn default() -> WorkMode {
        WorkMode::Normal
    }
}

impl FromStr for WorkMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "normal" => Ok(WorkMode::Normal),
            "sys" => Ok(WorkMode::Sys),
            "doc" => Ok(WorkMode::Doc),
            "subclass" => Ok(WorkMode::Subclass),
            "not_bound" => Ok(WorkMode::DisplayNotBound),
            _ => Err("Wrong work mode".into()),
        }
    }
}
