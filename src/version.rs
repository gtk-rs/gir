use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

/// Major, minor and patch version
#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u16, pub u16, pub u16);

impl Version {
    /// Convert a version number to a config guard
    ///
    /// When generating a builder pattern, properties could be from a super-type
    /// class/interface and so the version used there must be prefixed with
    /// the crate name from where the super-type originates from in case it
    /// is different from the main crate. For those cases you can pass
    /// the crate name as the `prefix` parameter
    pub fn to_cfg(self, prefix: Option<&str>) -> String {
        if let Some(p) = prefix {
            format!("feature = \"{}_{}\"", p, self.to_feature())
        } else {
            format!("feature = \"{}\"", self.to_feature())
        }
    }

    pub fn to_feature(self) -> String {
        match self {
            Self(major, 0, 0) => format!("v{major}"),
            Self(major, minor, 0) => format!("v{major}_{minor}"),
            Self(major, minor, patch) => format!("v{major}_{minor}_{patch}"),
        }
    }

    /// Returns `inner_version` if it is stricter than `outer_version`, `None`
    /// otherwise
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
    fn from_str(s: &str) -> Result<Self, String> {
        if s.contains('.') {
            let mut parts = s
                .splitn(4, '.')
                .map(str::parse)
                .take_while(Result::is_ok)
                .map(Result::unwrap);
            Ok(Self(
                parts.next().unwrap_or(0),
                parts.next().unwrap_or(0),
                parts.next().unwrap_or(0),
            ))
        } else {
            let val = s.parse::<u16>();
            Ok(Self(val.unwrap_or(0), 0, 0))
        }
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            Self(major, 0, 0) => write!(f, "{major}"),
            Self(major, minor, 0) => write!(f, "{major}.{minor}"),
            Self(major, minor, patch) => write!(f, "{major}.{minor}.{patch}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::Version;

    #[test]
    fn from_str_works() {
        assert_eq!(FromStr::from_str("1"), Ok(Version(1, 0, 0)));
        assert_eq!(FromStr::from_str("2.1"), Ok(Version(2, 1, 0)));
        assert_eq!(FromStr::from_str("3.2.1"), Ok(Version(3, 2, 1)));
        assert_eq!(FromStr::from_str("3.ff.1"), Ok(Version(3, 0, 0)));
    }

    #[test]
    fn parse_works() {
        assert_eq!("1".parse(), Ok(Version(1, 0, 0)));
    }

    #[test]
    fn ord() {
        assert!(Version(0, 0, 0) < Version(1, 2, 3));
        assert!(Version(1, 0, 0) < Version(1, 2, 3));
        assert!(Version(1, 2, 0) < Version(1, 2, 3));
        assert!(Version(1, 2, 3) == Version(1, 2, 3));
        assert!(Version(1, 0, 0) < Version(2, 0, 0));
        assert!(Version(3, 0, 0) == Version(3, 0, 0));
    }
}
