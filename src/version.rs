use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Version {
    // major, minor, patch
    Full(u16, u16, u16),
    Short(u16),
}

impl Version {
    pub fn to_cfg(self) -> String {
        format!("feature = \"{}\"", self.to_feature())
    }

    pub fn to_feature(self) -> String {
        use self::Version::*;
        match self {
            Full(major, minor, 0) => format!("v{}_{}", major, minor),
            Full(major, minor, patch) => format!("v{}_{}_{}", major, minor, patch),
            Short(major) => format!("v{}", major),
        }
    }

    /// Returns `inner_version` if it is stricter than `outer_version`, `None` otherwise
    pub fn if_stricter_than(
        inner_version: Option<Self>,
        outer_version: Option<Self>,
    ) -> Option<Self> {
        match (inner_version, outer_version) {
            (Some(inner_version), Some(outer_version)) if inner_version <= outer_version => None,
            (inner_version, _) => inner_version,
        }
    }
}

impl FromStr for Version {
    type Err = String;

    /// Parse a `Version` from a string.
    /// Currently always return Ok
    fn from_str(s: &str) -> Result<Version, String> {
        if s.contains('.') {
            let mut parts = s
                .splitn(4, '.')
                .map(str::parse)
                .take_while(Result::is_ok)
                .map(Result::unwrap);
            Ok(Version::Full(
                parts.next().unwrap_or(0),
                parts.next().unwrap_or(0),
                parts.next().unwrap_or(0),
            ))
        } else {
            let val = s.parse::<u16>();
            Ok(Version::Short(val.unwrap_or(0)))
        }
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        use self::Version::*;
        match *self {
            Full(major, minor, 0) => write!(f, "{}.{}", major, minor),
            Full(major, minor, patch) => write!(f, "{}.{}.{}", major, minor, patch),
            Short(major) => write!(f, "{}", major),
        }
    }
}

impl Default for Version {
    fn default() -> Version {
        Version::Full(0, 0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::Version::*;
    use std::str::FromStr;

    #[test]
    fn from_str_works() {
        assert_eq!(FromStr::from_str("1"), Ok(Short(1)));
        assert_eq!(FromStr::from_str("2.1"), Ok(Full(2, 1, 0)));
        assert_eq!(FromStr::from_str("3.2.1"), Ok(Full(3, 2, 1)));
        assert_eq!(FromStr::from_str("3.ff.1"), Ok(Full(3, 0, 0)));
    }

    #[test]
    fn parse_works() {
        assert_eq!("1".parse(), Ok(Short(1)));
    }

    #[test]
    fn ord() {
        assert!(Full(0, 0, 0) < Full(1, 2, 3));
        assert!(Full(1, 0, 0) < Full(1, 2, 3));
        assert!(Full(1, 2, 0) < Full(1, 2, 3));
        assert!(Full(1, 2, 3) == Full(1, 2, 3));
        assert!(Short(1) < Short(2));
        assert!(Short(3) == Short(3));
    }
}
