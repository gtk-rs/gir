use toml::Value;

use super::error::TomlHelper;
use super::ident::Ident;
use super::parsable::Parse;
use version::Version;

#[derive(Clone, Debug)]
pub struct Property {
    pub ident: Ident,
    //true - ignore this property,
    //false(default) - process this property
    pub ignore: bool,
    pub version: Option<Version>,
}

impl Parse for Property {
    fn parse(toml: &Value, object_name: &str) -> Option<Property> {
        let ident = match Ident::parse(toml, object_name, "property") {
            Some(ident) => ident,
            None => {
                error!(
                    "No 'name' or 'pattern' given for property for object {}",
                    object_name
                );
                return None;
            }
        };
        let ignore = toml.lookup("ignore")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);
        let version = toml.lookup("version")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());

        Some(Property {
            ident: ident,
            ignore: ignore,
            version: version,
        })
    }
}

impl AsRef<Ident> for Property {
    fn as_ref(&self) -> &Ident {
        &self.ident
    }
}

pub type Properties = Vec<Property>;

#[cfg(test)]
mod tests {
    use super::super::ident::Ident;
    use super::super::matchable::Matchable;
    use super::super::parsable::{Parsable, Parse};
    use super::*;
    use toml;
    use version::Version;

    fn properties_toml(input: &str) -> toml::Value {
        let mut value: toml::value::Table = toml::from_str(&input).unwrap();
        value.remove("f").unwrap()
    }

    fn toml(input: &str) -> toml::Value {
        let value = input.parse();
        assert!(value.is_ok());
        value.unwrap()
    }

    #[test]
    fn property_parse_ignore() {
        let toml = toml(
            r#"
name = "prop1"
ignore = true
"#,
        );
        let p = Property::parse(&toml, "a").unwrap();
        assert_eq!(p.ident, Ident::Name("prop1".into()));
        assert_eq!(p.ignore, true);
    }

    #[test]
    fn property_parse_version_default() {
        let toml = toml(
            r#"
name = "prop1"
"#,
        );
        let p = Property::parse(&toml, "a").unwrap();
        assert_eq!(p.version, None);
    }

    #[test]
    fn property_parse_version() {
        let toml = toml(
            r#"
name = "prop1"
version = "3.20"
"#,
        );
        let p = Property::parse(&toml, "a").unwrap();
        assert_eq!(p.version, Some(Version::Full(3, 20, 0)));
    }

    #[test]
    fn properties_parse_empty_for_none() {
        let props = Properties::parse(None, "a");
        assert!(props.is_empty());
    }

    #[test]
    fn properties_parse_matches() {
        let toml = properties_toml(
            r#"
[[f]]
name = "prop1"
[[f]]
name = "p1.5"
[[f]]
name = "prop2"
[[f]]
pattern = 'prop\d+'
"#,
        );
        let props = Properties::parse(Some(&toml), "a");
        assert_eq!(props.len(), 4);

        assert_eq!(props.matched("prop1").len(), 2);
        assert_eq!(props.matched("prop2").len(), 2);
        assert_eq!(props.matched("prop3").len(), 1);
        assert_eq!(props.matched("p1.5").len(), 1);
        assert_eq!(props.matched("none").len(), 0);
    }
}
