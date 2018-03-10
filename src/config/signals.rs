use std::str::FromStr;
use toml::Value;

use library;
use library::Nullable;
use super::error::TomlHelper;
use super::functions::Return;
use super::ident::Ident;
use super::parameter_matchable::Functionlike;
use super::parsable::{Parsable, Parse};
use version::Version;

#[derive(Clone, Copy, Debug)]
pub enum TransformationType {
    None,
    Borrow, //replace from_glib_none to from_glib_borrow
    //TODO: configure
    TreePath, //convert string to TreePath
}

impl FromStr for TransformationType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(TransformationType::None),
            "borrow" => Ok(TransformationType::Borrow),
            "treepath" => Ok(TransformationType::TreePath),
            _ => Err(format!("Wrong transformation \"{}\"", s)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Parameter {
    pub ident: Ident,
    pub nullable: Option<Nullable>,
    pub transformation: Option<TransformationType>,
    pub new_name: Option<String>,
}

impl Parse for Parameter {
    fn parse(toml: &Value, object_name: &str) -> Option<Parameter> {
        let ident = match Ident::parse(toml, object_name, "signal parameter") {
            Some(ident) => ident,
            None => {
                error!(
                    "No 'name' or 'pattern' given for parameter for object {}",
                    object_name
                );
                return None;
            }
        };
        toml.check_unwanted(
            &["nullable", "transformation", "new_name", "name", "pattern"],
            &format!("parameter {}", object_name),
        );

        let nullable = toml.lookup("nullable")
            .and_then(|val| val.as_bool())
            .map(Nullable);
        let transformation = toml.lookup("transformation")
            .and_then(|val| val.as_str())
            .and_then(|s| {
                TransformationType::from_str(s)
                    .map_err(|err| {
                        error!("{0}", err);
                        err
                    })
                    .ok()
            });
        let new_name = toml.lookup("new_name")
            .and_then(|val| val.as_str())
            .map(|s| s.to_owned());

        Some(Parameter {
            ident,
            nullable,
            transformation,
            new_name,
        })
    }
}

impl AsRef<Ident> for Parameter {
    fn as_ref(&self) -> &Ident {
        &self.ident
    }
}

pub type Parameters = Vec<Parameter>;

#[derive(Clone, Debug)]
pub struct Signal {
    pub ident: Ident,
    //true - ignore this signal
    //false(default) - process this signal
    pub ignore: bool,
    pub inhibit: bool,
    pub version: Option<Version>,
    pub parameters: Parameters,
    pub ret: Return,
    pub concurrency: library::Concurrency,
    pub doc_hidden: bool,
}

impl Signal {
    pub fn parse(
        toml: &Value,
        object_name: &str,
        concurrency: library::Concurrency,
    ) -> Option<Signal> {
        let ident = match Ident::parse(toml, object_name, "signal") {
            Some(ident) => ident,
            None => {
                error!(
                    "No 'name' or 'pattern' given for signal for object {}",
                    object_name
                );
                return None;
            }
        };
        toml.check_unwanted(
            &[
                "ignore",
                "inhibit",
                "version",
                "parameter",
                "return",
                "doc_hidden",
                "name",
                "pattern",
                "concurrency",
            ],
            &format!("signal {}", object_name),
        );

        let ignore = toml.lookup("ignore")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);

        let inhibit = toml.lookup("inhibit")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);
        let version = toml.lookup("version")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());
        let parameters = Parameters::parse(toml.lookup("parameter"), object_name);
        let ret = Return::parse(toml.lookup("return"));

        let concurrency = toml.lookup("concurrency")
            .and_then(|v| v.as_str())
            .and_then(|v| v.parse().ok())
            .unwrap_or(concurrency);

        let doc_hidden = toml.lookup("doc_hidden")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);

        Some(Signal {
            ident,
            ignore,
            inhibit,
            version,
            parameters,
            ret,
            concurrency,
            doc_hidden,
        })
    }
}

impl Functionlike for Signal {
    type Parameter = self::Parameter;

    fn parameters(&self) -> &[Self::Parameter] {
        &self.parameters
    }
}

impl AsRef<Ident> for Signal {
    fn as_ref(&self) -> &Ident {
        &self.ident
    }
}

pub type Signals = Vec<Signal>;

#[cfg(test)]
mod tests {
    use super::super::ident::Ident;
    use super::*;
    use toml;

    fn toml(input: &str) -> toml::Value {
        let value = input.parse::<toml::Value>();
        assert!(value.is_ok());
        value.unwrap()
    }

    #[test]
    fn signal_parse_default() {
        let toml = toml(
            r#"
name = "signal1"
"#,
        );
        let f = Signal::parse(&toml, "a", Default::default()).unwrap();
        assert_eq!(f.ident, Ident::Name("signal1".into()));
        assert_eq!(f.ignore, false);
    }

    #[test]
    fn signal_parse_ignore() {
        let toml = toml(
            r#"
name = "signal1"
ignore = true
"#,
        );
        let f = Signal::parse(&toml, "a", Default::default()).unwrap();
        assert_eq!(f.ignore, true);
    }
}
