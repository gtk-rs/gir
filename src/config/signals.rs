use library::Nullable;
use super::functions::Return;
use super::ident::Ident;
use super::matchable::Matchable;
use super::parsable::{Parsable, Parse};
use toml::Value;
use version::Version;

#[derive(Clone, Debug)]
pub struct Parameter {
    pub ident: Ident,
    pub nullable: Option<Nullable>,
}

impl Parse for Parameter {
    fn parse(toml: &Value, object_name: &str) -> Option<Parameter> {
        let ident = match Ident::parse(toml, object_name, "signal parameter") {
            Some(ident) => ident,
            None => {
                error!("No 'name' or 'pattern' given for parameter for object {}", object_name);
                return None
            }
        };
        let nullable = toml.lookup("nullable")
            .and_then(|val| val.as_bool())
            .map(|b| Nullable(b));

        Some(Parameter{
            ident: ident,
            nullable: nullable,
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
}

impl Parse for Signal {
    fn parse(toml: &Value, object_name: &str) -> Option<Signal> {
        let ident = match Ident::parse(toml, object_name, "signal") {
            Some(ident) => ident,
            None => {
                error!("No 'name' or 'pattern' given for signal for object {}", object_name);
                return None
            }
        };
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

        Some(Signal{
            ident: ident,
            ignore: ignore,
            inhibit: inhibit,
            version: version,
            parameters: parameters,
            ret: ret,
        })
    }
}

impl Signal {
    pub fn matched_parameters<'a>(signals: &[&'a Signal], parameter_name: &str) -> Vec<&'a Parameter> {
        let mut v = Vec::new();
        for f in signals {
            let pars = f.parameters.matched(parameter_name);
            v.extend_from_slice(&pars);
        }
        v
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
    use super::super::parsable::Parse;
    use super::*;
    use toml;

    fn toml(input: &str) -> toml::Value {
        let mut parser = toml::Parser::new(&input);
        let value = parser.parse();
        assert!(value.is_some());
        toml::Value::Table(value.unwrap())
    }

    #[test]
    fn signal_parse_default() {
        let toml = toml(r#"
name = "signal1"
"#);
        let f = Signal::parse(&toml, "a").unwrap();
        assert_eq!(f.ident, Ident::Name("signal1".into()));
        assert_eq!(f.ignore, false);
    }

    #[test]
    fn signal_parse_ignore() {
        let toml = toml(r#"
name = "signal1"
ignore = true
"#);
        let f = Signal::parse(&toml, "a").unwrap();
        assert_eq!(f.ignore, true);
    }
}
