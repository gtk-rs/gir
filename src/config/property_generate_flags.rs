use std::str::FromStr;

use bitflags::bitflags;

use super::error::TomlHelper;

bitflags! {
    pub struct PropertyGenerateFlags: u32 {
        const GET = 1;
        const SET = 2;
        const NOTIFY = 4;
    }
}

impl FromStr for PropertyGenerateFlags {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "get" => Ok(Self::GET),
            "set" => Ok(Self::SET),
            "notify" => Ok(Self::NOTIFY),
            _ => Err(format!("Wrong property generate flag \"{s}\"")),
        }
    }
}

impl PropertyGenerateFlags {
    pub fn parse_flags(toml: &toml::Value, option: &str) -> Result<Self, String> {
        let array = toml.as_result_vec(option)?;
        let mut val = Self::empty();
        for v in array {
            let s = v.as_str().ok_or(format!(
                "Invalid `{}` value element, expected a string, found {}",
                option,
                v.type_str()
            ))?;
            match Self::from_str(s) {
                Ok(v) => val |= v,
                e @ Err(_) => return e,
            }
        }
        Ok(val)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn parse(val: &str) -> Result<PropertyGenerateFlags, String> {
        let input = format!("generate={val}");
        let table: toml::Value = toml::from_str(&input).unwrap();
        let value = table.lookup("generate").unwrap();
        PropertyGenerateFlags::parse_flags(value, "generate")
    }

    #[test]
    fn parse_flags() {
        assert_eq!(parse(r#"["get"]"#).unwrap(), PropertyGenerateFlags::GET);
        assert_eq!(parse(r#"["set"]"#).unwrap(), PropertyGenerateFlags::SET);
        assert_eq!(
            parse(r#"["notify"]"#).unwrap(),
            PropertyGenerateFlags::NOTIFY
        );
        assert_eq!(
            parse(r#"["set","get"]"#).unwrap(),
            PropertyGenerateFlags::GET | PropertyGenerateFlags::SET
        );
        assert_eq!(
            parse(r#""get""#),
            Err("Invalid `generate` value, expected a array, found string".into())
        );
        assert_eq!(
            parse(r#"[true]"#),
            Err("Invalid `generate` value element, expected a string, found boolean".into())
        );
        assert_eq!(
            parse(r#"["bad"]"#),
            Err("Wrong property generate flag \"bad\"".into())
        );
        assert_eq!(
            parse(r#"["get", "bad"]"#),
            Err("Wrong property generate flag \"bad\"".into())
        );
    }
}
