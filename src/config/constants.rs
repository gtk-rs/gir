use log::error;
use toml::Value;

use super::{error::TomlHelper, gobjects::GStatus, ident::Ident, parsable::Parse};
use crate::version::Version;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Constant {
    pub ident: Ident,
    pub status: GStatus,
    pub version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub generate_doc: bool,
}

impl Parse for Constant {
    fn parse(toml: &Value, object_name: &str) -> Option<Self> {
        let ident = match Ident::parse(toml, object_name, "constant") {
            Some(ident) => ident,
            None => {
                error!(
                    "No 'name' or 'pattern' given for constant for object {}",
                    object_name
                );
                return None;
            }
        };
        toml.check_unwanted(
            &[
                "ignore",
                "manual",
                "name",
                "version",
                "cfg_condition",
                "pattern",
                "generate_doc",
            ],
            &format!("function {object_name}"),
        );

        let version = toml
            .lookup("version")
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
            status,
            version,
            cfg_condition,
            generate_doc,
        })
    }
}

impl AsRef<Ident> for Constant {
    fn as_ref(&self) -> &Ident {
        &self.ident
    }
}

pub type Constants = Vec<Constant>;

#[cfg(test)]
mod tests {
    use super::{super::parsable::Parse, *};

    fn toml(input: &str) -> ::toml::Value {
        let value = ::toml::from_str(input);
        assert!(value.is_ok());
        value.unwrap()
    }

    #[test]
    fn child_property_parse_generate_doc() {
        let r = toml(
            r#"
name = "prop"
generate_doc = false
"#,
        );
        let constant = Constant::parse(&r, "a").unwrap();
        assert!(!constant.generate_doc);

        // Ensure that the default value is "true".
        let r = toml(
            r#"
name = "prop"
"#,
        );
        let constant = Constant::parse(&r, "a").unwrap();
        assert!(constant.generate_doc);
    }
}
