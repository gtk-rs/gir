use std::str::FromStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkMode {
    Normal,          // generate widgets etc.
    Sys,             // generate -sys with FFI
    Doc,             // generate documentation file
    DisplayNotBound, // Show not bound types
}

impl WorkMode {
    pub fn is_normal(self) -> bool {
        match self {
            WorkMode::Normal => true,
            _ => false,
        }
    }

    pub fn is_generate_rust_files(self) -> bool {
        match self {
            WorkMode::Normal => true,
            WorkMode::Sys => true,
            _ => false,
        }
    }
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
            "not_bound" => Ok(WorkMode::DisplayNotBound),
            _ => Err(format!("Wrong work mode '{}'", s)),
        }
    }
}
