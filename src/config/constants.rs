use super::{error::TomlHelper, ident::Ident, parsable::Parse};
use crate::version::Version;
use toml::Value;

#[derive(Clone, Debug)]
pub struct Constant {
    pub ident: Ident,
    pub ignore: bool,
    pub version: Option<Version>,
    pub cfg_condition: Option<String>,
}

impl Parse for Constant {
    fn parse(toml: &Value, object_name: &str) -> Option<Constant> {
        let ident = match Ident::parse(toml, object_name, "function") {
            Some(ident) => ident,
            None => {
                error!(
                    "No 'name' or 'pattern' given for function for object {}",
                    object_name
                );
                return None;
            }
        };
        toml.check_unwanted(
            &["ignore", "name", "version", "cfg_condition", "pattern"],
            &format!("function {}", object_name),
        );

        let version = toml
            .lookup("version")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok());
        let cfg_condition = toml
            .lookup("cfg_condition")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        let ignore = toml
            .lookup("ignore")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        Some(Constant {
            ident,
            ignore,
            version,
            cfg_condition,
        })
    }
}

impl AsRef<Ident> for Constant {
    fn as_ref(&self) -> &Ident {
        &self.ident
    }
}

pub type Constants = Vec<Constant>;
