use super::ident::Ident;
use super::identables::Parse;
use toml::Value;

#[derive(Clone, Debug)]
pub struct Signal {
    pub ident: Ident,
    //true - ignore this signal
    //false(default) - process this signal
    pub ignore: bool,
    pub inhibit: bool,
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

        Some(Signal{
            ident: ident,
            ignore: ignore,
            inhibit: inhibit,
        })
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
    use super::super::identables::*;
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
