use library::Nullable;
use super::error::TomlHelper;
use super::ident::Ident;
use super::parameter_matchable::Functionlike;
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
    pub length_of: Option<String>,
}

impl Parse for Parameter {
    fn parse(toml: &Value, object_name: &str) -> Option<Parameter> {
        let ident = match Ident::parse(toml, object_name, "function parameter") {
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
            &["const", "nullable", "length_of", "name", "pattern"],
            &format!("function parameter {}", object_name),
        );

        let constant = toml.lookup("const")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);
        let nullable = toml.lookup("nullable")
            .and_then(|val| val.as_bool())
            .map(Nullable);
        let length_of = toml.lookup("length_of")
            .and_then(|val| val.as_str())
            .map(|s| if s == "return" { "" } else { s })
            .map(ToOwned::to_owned);

        Some(Parameter {
            ident: ident,
            constant: constant,
            nullable: nullable,
            length_of: length_of,
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
    pub bool_return_is_error: Option<String>,
}

impl Return {
    pub fn parse(toml: Option<&Value>) -> Return {
        if let Some(v) = toml {
            v.check_unwanted(&["nullable", "bool_return_is_error"], "return");

            let nullable = v.lookup("nullable").and_then(|v| v.as_bool()).map(Nullable);
            let bool_return_is_error = v.lookup("bool_return_is_error")
                .and_then(|v| v.as_str())
                .map(|m| m.to_owned());
            Return {
                nullable: nullable,
                bool_return_is_error: bool_return_is_error,
            }
        } else {
            Return {
                nullable: None,
                bool_return_is_error: None,
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
    pub doc_hidden: bool,
    pub is_windows_utf8: bool,
    pub disable_length_detect: bool,
}

impl Parse for Function {
    fn parse(toml: &Value, object_name: &str) -> Option<Function> {
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
            &[
                "ignore",
                "version",
                "cfg_condition",
                "parameter",
                "return",
                "name",
                "doc_hidden",
                "is_windows_utf8",
                "disable_length_detect",
                "pattern",
            ],
            &format!("function {}", object_name),
        );

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
        let doc_hidden = toml.lookup("doc_hidden")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);
        let is_windows_utf8 = toml.lookup("is_windows_utf8")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);
        let disable_length_detect = toml.lookup("disable_length_detect")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);

        Some(Function {
            ident: ident,
            ignore: ignore,
            version: version,
            parameters: parameters,
            ret: ret,
            cfg_condition: cfg_condition,
            doc_hidden: doc_hidden,
            is_windows_utf8: is_windows_utf8,
            disable_length_detect: disable_length_detect,
        })
    }
}

impl Functionlike for Function {
    type Parameter = self::Parameter;

    fn parameters(&self) -> &[Self::Parameter] {
        &self.parameters
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
    use super::super::parameter_matchable::ParameterMatchable;
    use super::*;
    use toml;
    use version::Version;

    fn functions_toml(input: &str) -> toml::Value {
        let mut value: toml::value::Table = toml::from_str(&input).unwrap();
        value.remove("f").unwrap()
    }

    fn toml(input: &str) -> toml::Value {
        let value = toml::from_str(&input);
        assert!(value.is_ok());
        value.unwrap()
    }

    #[test]
    fn function_parse_ignore() {
        let toml = toml(
            r#"
name = "func1"
ignore = true
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ident, Ident::Name("func1".into()));
        assert_eq!(f.ignore, true);
    }

    #[test]
    fn function_parse_version_default() {
        let toml = toml(
            r#"
name = "func1"
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.version, None);
    }

    #[test]
    fn function_parse_version() {
        let toml = toml(
            r#"
name = "func1"
version = "3.20"
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.version, Some(Version::Full(3, 20, 0)));
    }

    #[test]
    fn function_parse_cfg_condition_default() {
        let toml = toml(
            r#"
name = "func1"
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.cfg_condition, None);
    }

    #[test]
    fn function_parse_cfg_condition() {
        let toml = toml(
            r#"
name = "func1"
cfg_condition = 'unix'
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.cfg_condition, Some("unix".to_string()));
    }

    #[test]
    fn function_parse_return_nullable_default1() {
        let toml = toml(
            r#"
name = "func1"
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ret.nullable, None);
    }

    #[test]
    fn function_parse_return_nullable_default2() {
        let toml = toml(
            r#"
name = "func1"
[return]
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ret.nullable, None);
    }

    #[test]
    fn function_parse_parameters() {
        let toml = toml(
            r#"
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
"#,
        );
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
        let toml = toml(
            r#"
name = "func1"
[return]
nullable = false
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ret.nullable, Some(Nullable(false)));
    }

    #[test]
    fn function_parse_return_nullable_true() {
        let toml = toml(
            r#"
name = "func1"
[return]
nullable = true
"#,
        );
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
        let toml = functions_toml(
            r#"
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
"#,
        );
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
        let toml = functions_toml(
            r#"
[[f]]
name = "func1"
[[f]]
name = "f1.5"
[[f]]
name = "func2"
[[f]]
pattern = 'func\d+'
"#,
        );
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
        let toml = functions_toml(
            r#"
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
"#,
        );
        let fns = Functions::parse(Some(&toml), "a");
        assert_eq!(fns.len(), 2);
        let m = fns.matched("func");
        assert_eq!(m.len(), 2);

        assert_eq!(m.matched_parameters("param").len(), 0);
        assert_eq!(m.matched_parameters("par1").len(), 3);
        assert_eq!(m.matched_parameters("par2").len(), 4);
        assert_eq!(m.matched_parameters("par3").len(), 3);
        assert_eq!(m.matched_parameters("par4").len(), 2);
    }
}
