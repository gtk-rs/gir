use super::ident::Ident;
use super::parsable::Parse;
use toml::Value;
use super::error::TomlHelper;
use version::Version;

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

        let version = toml.lookup("version")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());
        let cfg_condition = toml.lookup("cfg_condition")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        let ignore = toml.lookup("ignore")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);

        Some(Constant {
            ident: ident,
            ignore: ignore,
            version: version,
            cfg_condition: cfg_condition,
        })
    }
}

impl AsRef<Ident> for Constant {
    fn as_ref(&self) -> &Ident {
        &self.ident
    }
}

pub type Constants = Vec<Constant>;
