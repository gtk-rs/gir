use log::error;
use toml::Value;

use super::{error::TomlHelper, parsable::Parse};

#[derive(Clone, Debug)]
pub struct Derive {
    pub names: Vec<String>,
    pub cfg_condition: Option<String>,
}

impl Parse for Derive {
    fn parse(toml: &Value, object_name: &str) -> Option<Self> {
        let names = match toml.lookup("name").and_then(Value::as_str) {
            Some(names) => names,
            None => {
                error!("No 'name' given for derive for object {}", object_name);
                return None;
            }
        };
        toml.check_unwanted(&["name", "cfg_condition"], &format!("derive {object_name}"));

        let cfg_condition = toml
            .lookup("cfg_condition")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);

        let mut names_vec = Vec::new();
        for name in names.split(',') {
            names_vec.push(name.trim().into());
        }

        Some(Self {
            names: names_vec,
            cfg_condition,
        })
    }
}

pub type Derives = Vec<Derive>;
