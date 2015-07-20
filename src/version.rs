use std::cmp::{Ord, Ordering, PartialOrd};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// major, minor, patch
pub struct Version(pub u16, pub u16, pub u16);

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
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.0.cmp(&other.0) {
            Ordering::Equal => {
                match self.1.cmp(&other.1) {
                    Ordering::Equal => self.2.cmp(&other.2),
                    x => x,
                }
            }
            x => x,
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
