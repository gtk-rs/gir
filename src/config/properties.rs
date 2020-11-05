use super::{
    error::TomlHelper, gobjects::GStatus, ident::Ident, parsable::Parse,
    property_generate_flags::PropertyGenerateFlags,
};
use crate::version::Version;
use log::error;
use toml::Value;

#[derive(Clone, Debug)]
pub struct Property {
    pub ident: Ident,
    pub status: GStatus,
    pub version: Option<Version>,
    pub generate: Option<PropertyGenerateFlags>,
    pub doc_trait_name: Option<String>,
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

        toml.check_unwanted(
            &[
                "ignore",
                "manual",
                "version",
                "name",
                "pattern",
                "generate",
                "doc_trait_name",
            ],
            &format!("property {}", object_name),
        );

        let status = {
            if toml
                .lookup("ignore")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                GStatus::Ignore
            } else if toml
                .lookup("manual")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                GStatus::Manual
            } else {
                GStatus::Generate
            }
        };
        let version = toml
            .lookup("version")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok());
        let generate = toml.lookup("generate").and_then(|v| {
            PropertyGenerateFlags::parse_flags(v, "generate")
                .map_err(|e| error!("{} for object {}", e, object_name))
                .ok()
        });
        let doc_trait_name = toml
            .lookup("doc_trait_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        Some(Property {
            ident,
            status,
            version,
            generate,
            doc_trait_name,
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
    use super::{
        super::{
            ident::Ident,
            matchable::Matchable,
            parsable::{Parsable, Parse},
        },
        *,
    };
    use crate::version::Version;

    fn properties_toml(input: &str) -> ::toml::Value {
        let mut value: ::toml::value::Table = ::toml::from_str(&input).unwrap();
        value.remove("f").unwrap()
    }

    fn toml(input: &str) -> ::toml::Value {
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
        assert!(p.status.ignored());
    }

    #[test]
    fn property_parse_manual() {
        let toml = toml(
            r#"
name = "prop1"
manual = true
"#,
        );
        let p = Property::parse(&toml, "a").unwrap();
        assert_eq!(p.ident, Ident::Name("prop1".into()));
        assert!(p.status.manual());
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
        assert!(p.status.need_generate());
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
