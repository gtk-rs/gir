use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
// major, minor, patch
pub struct Version(pub u16, pub u16, pub u16);

impl Version {
    pub fn to_cfg(&self) -> String {
        format!("feature = \"{}\"", self)
    }
}

impl FromStr for Version {
    type Err = String;

    /// Parse a `Version` from a string.
    fn from_str(s: &str) -> Result<Version, String> {
        let mut parts = s.splitn(4, '.')
            .map(|s| s.parse())
            .take_while(Result::is_ok)
            .map(Result::unwrap);
        Ok(Version(parts.next().unwrap_or(0),
            parts.next().unwrap_or(0), parts.next().unwrap_or(0)))
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            Version(major, minor, 0) => write!(f, "{}.{}", major, minor),
            Version(major, minor, patch) => write!(f, "{}.{}.{}", major, minor, patch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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
    }
}
