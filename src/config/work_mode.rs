use std::str::FromStr;

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkMode {
    #[default]
    Normal, // generate widgets etc.
    Sys,             // generate -sys with FFI
    Doc,             // generate documentation file
    DisplayNotBound, // Show not bound types
}

impl WorkMode {
    pub fn is_normal(self) -> bool {
        matches!(self, Self::Normal)
    }

    pub fn is_generate_rust_files(self) -> bool {
        matches!(self, Self::Normal | Self::Sys)
    }
}

impl FromStr for WorkMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "normal" => Ok(Self::Normal),
            "sys" => Ok(Self::Sys),
            "doc" => Ok(Self::Doc),
            "not_bound" => Ok(Self::DisplayNotBound),
            _ => Err(format!("Wrong work mode '{s}'")),
        }
    }
}
