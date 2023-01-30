use log::error;
use toml::Value;

use super::{error::TomlHelper, gobjects::GStatus, ident::Ident, parsable::Parse};
use crate::version::Version;

#[derive(Clone, Debug)]
pub struct Member {
    pub ident: Ident,
    // some enum variants have multiple names
    pub alias: bool,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub status: GStatus,
    pub cfg_condition: Option<String>,
    pub generate_doc: bool,
}

impl Parse for Member {
    fn parse(toml: &Value, object_name: &str) -> Option<Self> {
        let ident = match Ident::parse(toml, object_name, "member") {
            Some(ident) => ident,
            None => {
                error!(
                    "No 'name' or 'pattern' given for member for object {}",
                    object_name
                );
                return None;
            }
        };

        toml.check_unwanted(
            &[
                "alias",
                "version",
                "name",
                "pattern",
                "ignore",
                "manual",
                "cfg_condition",
                "generate_doc",
            ],
            &format!("member {object_name}"),
        );

        let alias = toml
            .lookup("alias")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let version = toml
            .lookup("version")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok());
        let deprecated_version = toml
            .lookup("deprecated_version")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok());
        let cfg_condition = toml
            .lookup("cfg_condition")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

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
        let generate_doc = toml
            .lookup("generate_doc")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        Some(Self {
            ident,
            alias,
            version,
            deprecated_version,
            status,
            cfg_condition,
            generate_doc,
        })
    }
}

impl AsRef<Ident> for Member {
    fn as_ref(&self) -> &Ident {
        &self.ident
    }
}

pub type Members = Vec<Member>;

#[cfg(test)]
mod tests {
    use super::{
        super::{ident::Ident, parsable::Parse},
        *,
    };
    use crate::version::Version;

    fn toml(input: &str) -> ::toml::Value {
        let value = ::toml::from_str(input);
        assert!(value.is_ok());
        value.unwrap()
    }

    #[test]
    fn member_parse_alias() {
        let toml = toml(
            r#"
name = "name1"
alias = true
"#,
        );
        let f = Member::parse(&toml, "a").unwrap();
        assert_eq!(f.ident, Ident::Name("name1".into()));
        assert!(f.alias);
    }

    #[test]
    fn member_parse_version_default() {
        let toml = toml(
            r#"
name = "name1"
"#,
        );
        let f = Member::parse(&toml, "a").unwrap();
        assert_eq!(f.version, None);
    }

    #[test]
    fn member_parse_version() {
        let toml = toml(
            r#"
name = "name1"
version = "3.20"
"#,
        );
        let f = Member::parse(&toml, "a").unwrap();
        assert_eq!(f.version, Some(Version(3, 20, 0)));
    }

    #[test]
    fn member_parse_generate_doc() {
        let r = toml(
            r#"
name = "name1"
generate_doc = false
"#,
        );
        let f = Member::parse(&r, "a").unwrap();
        assert!(!f.generate_doc);

        // We now ensure that the default value is true.
        let r = toml(
            r#"
name = "name1"
"#,
        );
        let f = Member::parse(&r, "a").unwrap();
        assert!(f.generate_doc);
    }
}
