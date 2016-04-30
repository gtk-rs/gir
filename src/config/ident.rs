use regex::Regex;
use toml::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ident {
    Name(String),
    Pattern(Regex),
}

impl Ident {
    pub fn parse(toml: &Value, object_name: &str, what: &str) -> Option<Ident> {
        match toml.lookup("pattern").and_then(|v| v.as_str()) {
            Some(s) => Regex::new(&format!("^{}$",s))
                .map(|r| Ident::Pattern(r))
                .map_err(|e| {
                    error!("Bad pattern `{}` in {} for `{}`: {}", s, what, object_name, e);
                    e
                })
                .ok(),
            None => toml.lookup("name")
                .and_then(|val| val.as_str())
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
