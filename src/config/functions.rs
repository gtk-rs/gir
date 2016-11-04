use library::Nullable;
use super::ident::Ident;
use super::matchable::Matchable;
use super::parsable::{Parsable, Parse};
use toml::Value;
use version::Version;

#[derive(Clone, Debug)]
pub struct Parameter {
    pub ident: Ident,
    //true - parameter don't changed in ffi function,
    //false(default) - parameter can be changed in ffi function
    pub constant: bool,
    pub nullable: Option<Nullable>,
}

impl Parse for Parameter {
    fn parse(toml: &Value, object_name: &str) -> Option<Parameter> {
        let ident = match Ident::parse(toml, object_name, "function parameter") {
            Some(ident) => ident,
            None => {
                error!("No 'name' or 'pattern' given for parameter for object {}", object_name);
                return None
            }
        };
        let constant = toml.lookup("const")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);
        let nullable = toml.lookup("nullable")
            .and_then(|val| val.as_bool())
            .map(|b| Nullable(b));

        Some(Parameter{
            ident: ident,
            constant: constant,
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
pub struct Return {
    pub nullable: Option<Nullable>,
}

impl Return {
    pub fn parse(toml: Option<&Value>) -> Return {
        if let Some(v) = toml {
            let nullable = v.lookup("nullable")
                .and_then(|v| v.as_bool())
                .map(|b| Nullable(b));
            Return {
                nullable: nullable,
            }
        } else {
            Return {
                nullable: None,
            }
        }

    }
}

#[derive(Clone, Debug)]
pub struct Function {
    pub ident: Ident,
    //true - ignore this function,
    //false(default) - process this function
    pub ignore: bool,
    pub version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub parameters: Parameters,
    pub ret: Return,
}

impl Parse for Function {
    fn parse(toml: &Value, object_name: &str) -> Option<Function> {
        let ident = match Ident::parse(toml, object_name, "function") {
            Some(ident) => ident,
            None => {
                error!("No 'name' or 'pattern' given for function for object {}", object_name);
                return None
            }
        };
        let ignore = toml.lookup("ignore")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);
        let version = toml.lookup("version")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok());
        let cfg_condition = toml.lookup("cfg_condition")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        let parameters = Parameters::parse(toml.lookup("parameter"), object_name);
        let ret = Return::parse(toml.lookup("return"));

        Some(Function{
            ident: ident,
            ignore: ignore,
            version: version,
            parameters: parameters,
            ret: ret,
            cfg_condition: cfg_condition,
        })
    }
}

impl Function {
    pub fn matched_parameters<'a>(functions: &[&'a Function], parameter_name: &str) -> Vec<&'a Parameter> {
        let mut v = Vec::new();
        for f in functions {
            let pars = f.parameters.matched(parameter_name);
            v.extend_from_slice(&pars);
        }
        v
    }
}

impl AsRef<Ident> for Function {
    fn as_ref(&self) -> &Ident {
        &self.ident
    }
}

pub type Functions = Vec<Function>;

#[cfg(test)]
mod tests {
    use library::Nullable;
    use super::super::ident::Ident;
    use super::super::matchable::Matchable;
    use super::super::parsable::{Parsable, Parse};
    use super::*;
    use toml;
    use version::Version;

    fn functions_toml(input: &str) -> toml::Value {
        let mut parser = toml::Parser::new(&input);
        let value = parser.parse();
        assert!(value.is_some());
        value.unwrap().remove("f").unwrap()
    }

    fn toml(input: &str) -> toml::Value {
        let mut parser = toml::Parser::new(&input);
        let value = parser.parse();
        assert!(value.is_some());
        toml::Value::Table(value.unwrap())
    }

    #[test]
    fn function_parse_ignore() {
        let toml = toml(r#"
name = "func1"
ignore = true
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ident, Ident::Name("func1".into()));
        assert_eq!(f.ignore, true);
    }

    #[test]
    fn function_parse_version_default() {
        let toml = toml(r#"
name = "func1"
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.version, None);
    }

    #[test]
    fn function_parse_version() {
        let toml = toml(r#"
name = "func1"
version = "3.20"
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.version, Some(Version::Full(3, 20, 0)));
    }

    #[test]
    fn function_parse_cfg_condition_default() {
        let toml = toml(r#"
name = "func1"
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.cfg_condition, None);
    }

    #[test]
    fn function_parse_cfg_condition() {
        let toml = toml(r#"
name = "func1"
cfg_condition = 'unix'
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.cfg_condition, Some("unix".to_string()));
    }

    #[test]
    fn function_parse_return_nullable_default1() {
        let toml = toml(r#"
name = "func1"
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ret.nullable, None);
    }

    #[test]
    fn function_parse_return_nullable_default2() {
        let toml = toml(r#"
name = "func1"
[return]
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ret.nullable, None);
    }

    #[test]
    fn function_parse_parameters() {
        let toml = toml(r#"
name = "func1"
[[parameter]]
name = "par1"
[[parameter]]
name = "par2"
const = false
nullable = false
[[parameter]]
name = "par3"
const = true
nullable = true
[[parameter]]
pattern = "par4"
const = true
"#);
        let f = Function::parse(&toml, "a").unwrap();
        let pars = f.parameters;
        assert_eq!(pars.len(), 4);
        assert_eq!(pars[0].ident, Ident::Name("par1".into()));
        assert_eq!(pars[0].constant, false);
        assert_eq!(pars[0].nullable, None);
        assert_eq!(pars[1].ident, Ident::Name("par2".into()));
        assert_eq!(pars[1].constant, false);
        assert_eq!(pars[1].nullable, Some(Nullable(false)));
        assert_eq!(pars[2].ident, Ident::Name("par3".into()));
        assert_eq!(pars[2].constant, true);
        assert_eq!(pars[2].nullable, Some(Nullable(true)));
        if let Ident::Pattern(_) = pars[3].ident {
        } else {
            assert!(false, "Pattern don't parsed");
        }
        assert_eq!(pars[3].constant, true);
        assert_eq!(pars[3].nullable, None);
    }

    #[test]
    fn function_parse_return_nullable_false() {
        let toml = toml(r#"
name = "func1"
[return]
nullable = false
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ret.nullable, Some(Nullable(false)));
    }

    #[test]
    fn function_parse_return_nullable_true() {
        let toml = toml(r#"
name = "func1"
[return]
nullable = true
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ret.nullable, Some(Nullable(true)));
    }

    #[test]
    fn functions_parse_empty_for_none() {
        let fns = Functions::parse(None, "a");
        assert!(fns.is_empty());
    }

    #[test]
    fn functions_parse_ident() {
        let toml = functions_toml(r#"
[[f]]
name = "func1"
[[f]]
not_name = "func1.5"
[[f]]
name = "func2"
[[f]]
pattern = 'func3\w+'
[[f]]
pattern = 'bad_func4[\w+'
"#);
        let fns = Functions::parse(Some(&toml), "a");
        assert_eq!(fns.len(), 3);
        assert_eq!(fns[0].ident, Ident::Name("func1".into()));
        assert_eq!(fns[1].ident, Ident::Name("func2".into()));
        if let Ident::Pattern(_) = fns[2].ident {
        } else {
            assert!(false, "Pattern don't parsed");
        }
    }

    #[test]
    fn functions_parse_matches() {
        let toml = functions_toml(r#"
[[f]]
name = "func1"
[[f]]
name = "f1.5"
[[f]]
name = "func2"
[[f]]
pattern = 'func\d+'
"#);
        let fns = Functions::parse(Some(&toml), "a");
        assert_eq!(fns.len(), 4);

        assert_eq!(fns.matched("func1").len(), 2);
        assert_eq!(fns.matched("func2").len(), 2);
        assert_eq!(fns.matched("func3").len(), 1);
        assert_eq!(fns.matched("f1.5").len(), 1);
        assert_eq!(fns.matched("none").len(), 0);
    }

    #[test]
    fn functions_parse_matched_parameters() {
        let toml = functions_toml(r#"
[[f]]
name = "func"
[[f.parameter]]
name="par1"
[[f.parameter]]
name="par2"
[[f.parameter]]
pattern='par\d+'
[[f]]
name = "func"
[[f.parameter]]
name="par2"
[[f.parameter]]
name="par3"
[[f.parameter]]
pattern='par\d+'
"#);
        let fns = Functions::parse(Some(&toml), "a");
        assert_eq!(fns.len(), 2);
        let m = fns.matched("func");
        assert_eq!(m.len(), 2);

        assert_eq!(Function::matched_parameters(&m, "param").len(), 0);
        assert_eq!(Function::matched_parameters(&m, "par1").len(), 3);
        assert_eq!(Function::matched_parameters(&m, "par2").len(), 4);
        assert_eq!(Function::matched_parameters(&m, "par3").len(), 3);
        assert_eq!(Function::matched_parameters(&m, "par4").len(), 2);
    }
}
