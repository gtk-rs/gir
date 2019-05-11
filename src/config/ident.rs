use log::error;
use regex::Regex;
use toml::Value;

use super::error::TomlHelper;

#[derive(Clone, Debug)]
pub enum Ident {
    Name(String),
    Pattern(Regex),
}

impl PartialEq for Ident {
    fn eq(&self, other: &Ident) -> bool {
        pub use self::Ident::*;
        match (self, other) {
            (&Name(ref s1), &Name(ref s2)) => s1 == s2,
            (&Pattern(ref r1), &Pattern(ref r2)) => r1.as_str() == r2.as_str(),
            _ => false,
        }
    }
}

impl Eq for Ident {}

impl Ident {
    pub fn parse(toml: &Value, object_name: &str, what: &str) -> Option<Ident> {
        match toml.lookup("pattern").and_then(Value::as_str) {
            Some(s) => Regex::new(&format!("^{}$", s))
                .map(Ident::Pattern)
                .map_err(|e| {
                    error!(
                        "Bad pattern `{}` in {} for `{}`: {}",
                        s, what, object_name, e
                    );
                    e
                })
                .ok(),
            None => toml
                .lookup("name")
                .and_then(Value::as_str)
                .map(|s| Ident::Name(s.into())),
        }
    }

    pub fn is_match(&self, name: &str) -> bool {
        use self::Ident::*;
        match *self {
            Name(ref n) => name == n,
            Pattern(ref regex) => regex.is_match(name),
        }
    }
}
