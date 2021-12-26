use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Visibility {
    Public,
    Crate,
    Super,
    Private,
}

impl Visibility {
    pub fn is_public(&self) -> bool {
        matches!(self, Self::Public)
    }

    pub fn export_visibility(&self) -> String {
        match self {
            Self::Public => "pub",
            Self::Private => "",
            Self::Crate => "pub(crate)",
            Self::Super => "pub(super)",
        }
        .to_owned()
    }
}

impl Default for Visibility {
    fn default() -> Self {
        Self::Public
    }
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => write!(f, "pub"),
            Self::Crate => write!(f, "pub(crate)"),
            Self::Private => write!(f, ""),
            Self::Super => write!(f, "pub(super)"),
        }
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
            e => Err(ParseVisibilityError(format!(
                "Wrong visibility type '{}'",
                e
            ))),
        }
    }
}
