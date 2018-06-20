use super::parsable::Parse;
use toml::Value;
use super::error::TomlHelper;

#[derive(Clone, Debug)]
pub struct Derive {
    pub name: String,
    pub cfg_condition: Option<String>,
}

impl Parse for Derive {
    fn parse(toml: &Value, object_name: &str) -> Option<Derive> {
        let name = match toml.lookup("name").and_then(|v| v.as_str()) {
            Some(name) => name.to_owned(),
            None => {
                error!(
                    "No 'name' given for derive for object {}",
                    object_name
                );
                return None;
            }
        };
        toml.check_unwanted(
            &["name", "cfg_condition"],
            &format!("derive {}", object_name),
        );

        let cfg_condition = toml.lookup("cfg_condition")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        Some(Derive {
            name,
            cfg_condition,
        })
    }
}

pub type Derives = Vec<Derive>;
