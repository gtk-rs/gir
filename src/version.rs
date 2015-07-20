use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq)]
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
}
