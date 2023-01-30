use std::{fmt, str::FromStr};

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq)]
pub enum Visibility {
    #[default]
    Public,
    Crate,
    Super,
    Private,
}

impl Visibility {
    pub fn is_public(self) -> bool {
        self == Self::Public
    }

    pub fn export_visibility(self) -> &'static str {
        match self {
            Self::Public => "pub",
            Self::Private => "",
            Self::Crate => "pub(crate)",
            Self::Super => "pub(super)",
        }
    }
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.export_visibility())
    }
}

#[derive(Debug)]
pub struct ParseVisibilityError(String);

impl std::error::Error for ParseVisibilityError {}

impl fmt::Display for ParseVisibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Visibility {
    type Err = ParseVisibilityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pub" => Ok(Self::Public),
            "super" => Ok(Self::Super),
            "private" => Ok(Self::Private),
            "crate" => Ok(Self::Crate),
            e => Err(ParseVisibilityError(format!("Wrong visibility type '{e}'"))),
        }
    }
}
