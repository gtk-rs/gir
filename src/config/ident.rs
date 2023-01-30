use std::fmt;

use log::error;
use regex::Regex;
use toml::Value;

use super::error::TomlHelper;

#[derive(Clone, Debug)]
pub enum Ident {
    Name(String),
    Pattern(Box<Regex>),
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(name) => f.write_str(name),
            Self::Pattern(regex) => write!(f, "Regex {regex}"),
        }
    }
}

impl PartialEq for Ident {
    fn eq(&self, other: &Ident) -> bool {
        match (self, other) {
            (Self::Name(s1), Self::Name(s2)) => s1 == s2,
            (Self::Pattern(r1), Self::Pattern(r2)) => r1.as_str() == r2.as_str(),
            _ => false,
        }
    }
}

impl Eq for Ident {}

impl Ident {
    pub fn parse(toml: &Value, object_name: &str, what: &str) -> Option<Self> {
        match toml.lookup("pattern").and_then(Value::as_str) {
            Some(s) => Regex::new(&format!("^{s}$"))
                .map(Box::new)
                .map(Self::Pattern)
                .map_err(|e| {
                    error!(
                        "Bad pattern `{}` in {} for `{}`: {}",
                        s, what, object_name, e
                    );
                    e
                })
                .ok(),
            None => match toml.lookup("name").and_then(Value::as_str) {
                Some(name) => {
                    if name.contains(['.', '+', '*'].as_ref()) {
                        error!(
                            "Should be `pattern` instead of `name` in {} for `{}`",
                            what, object_name
                        );
                        None
                    } else {
                        Some(Self::Name(name.into()))
                    }
                }
                None => None,
            },
        }
    }

    pub fn is_match(&self, name: &str) -> bool {
        use self::Ident::*;
        match self {
            Name(n) => name == n,
            Pattern(regex) => regex.is_match(name),
        }
    }
}
