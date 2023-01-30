use log::error;
use toml::Value;

use super::{
    error::TomlHelper, gobjects::GStatus, ident::Ident, parsable::Parse,
    property_generate_flags::PropertyGenerateFlags,
};
use crate::version::Version;

#[derive(Clone, Debug)]
pub struct Property {
    pub ident: Ident,
    pub status: GStatus,
    pub version: Option<Version>,
    pub generate: Option<PropertyGenerateFlags>,
    pub bypass_auto_rename: bool,
    pub doc_trait_name: Option<String>,
    pub generate_doc: bool,
}

impl Parse for Property {
    fn parse(toml: &Value, object_name: &str) -> Option<Self> {
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
                "bypass_auto_rename",
                "doc_trait_name",
                "generate_doc",
            ],
            &format!("property {object_name}"),
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
        let bypass_auto_rename = toml
            .lookup("bypass_auto_rename")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let doc_trait_name = toml
            .lookup("doc_trait_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let generate_doc = toml
            .lookup("generate_doc")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        Some(Self {
            ident,
            status,
            version,
            generate,
            bypass_auto_rename,
            doc_trait_name,
            generate_doc,
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
        let mut value: ::toml::value::Table = ::toml::from_str(input).unwrap();
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
    fn property_bypass_auto_rename() {
        let toml = toml(
            r#"
name = "prop1"
bypass_auto_rename = true
"#,
        );
        let f = Property::parse(&toml, "a").unwrap();
        assert_eq!(f.ident, Ident::Name("prop1".into()));
        assert!(f.bypass_auto_rename);
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
        assert_eq!(p.version, Some(Version(3, 20, 0)));
    }

    #[test]
    fn property_generate_doc() {
        let r = toml(
            r#"
name = "prop"
generate_doc = false
"#,
        );
        let p = Property::parse(&r, "a").unwrap();
        assert!(!p.generate_doc);

        // Ensure that the default value is "true".
        let r = toml(
            r#"
name = "prop"
"#,
        );
        let p = Property::parse(&r, "a").unwrap();
        assert!(p.generate_doc);
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
        assert_eq!(props.len(), 3);

        assert_eq!(props.matched("prop1").len(), 2);
        assert_eq!(props.matched("prop2").len(), 2);
        assert_eq!(props.matched("prop3").len(), 1);
        // "p1.5" is an invalid name
        assert_eq!(props.matched("p1.5").len(), 0);
        assert_eq!(props.matched("none").len(), 0);
    }
}
