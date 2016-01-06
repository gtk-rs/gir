use library::Nullable;
use regex::*;
use std::vec::Vec;
use toml::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Ident {
    Name(String),
    Pattern(Regex),
}

impl Ident {
    pub fn parse(toml: &Value, object_name: &str, is_parameter: bool) -> Option<Ident> {
        match toml.lookup("pattern").and_then(|v| v.as_str()) {
            Some(s) => Regex::new(s)
                .map(|r| Ident::Pattern(r))
                .map_err(|e| {
                    if is_parameter {
                        error!("Bad pattern '{}' in functions parameter for '{}': {}", s, object_name, e);
                    } else {
                        error!("Bad pattern '{}' in function for '{}': {}", s, object_name, e);
                    }
                    e
                })
                .ok(),
            None => toml.lookup("name")
                .and_then(|val| val.as_str())
                .map(|s| Ident::Name(s.into())),
        }
    }

    fn is_match(&self, name: &str) -> bool {
        use self::Ident::*;
        match *self {
            Name(ref n) => name == n,
            Pattern(ref regex) => regex.is_match(name),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Parameter {
    pub ident: Ident,
    //true - parameter don't changed in ffi function,
    //false(default) - parameter can be changed in ffi function
    pub constant: bool,
    pub nullable: Option<Nullable>,
}

impl Parameter {
    pub fn parse(toml: &Value, object_name: &str) -> Option<Parameter> {
        let ident = match Ident::parse(toml, object_name, true) {
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

#[derive(Clone, Debug)]
pub struct Parameters(Vec<Parameter>);

impl Parameters {
    pub fn parse(toml: Option<&Value>, object_name: &str) -> Parameters {
        let mut v = Vec::new();
        if let Some(pars) = toml.and_then(|val| val.as_slice()) {
            for par in pars {
                if let Some(par) = Parameter::parse(par, object_name) {
                    v.push(par);
                }
            }
        }

        Parameters(v)
    }

    pub fn matched(&self, parameter_name: &str) -> Vec<&Parameter> {
        self.0.iter().filter(|p| p.ident.is_match(parameter_name)).collect()
    }

    #[cfg(test)]
    fn vec(&self) -> &Vec<Parameter> {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct Return {
    //true(default) - function can be nullable,
    //false - function is nonnullable
    pub nullable: bool,
}

impl Return {
    pub fn parse(toml: Option<&Value>) -> Return {
        if let Some(v) = toml {
            let nullable = v.lookup("nullable")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            Return {
                nullable: nullable,
            }
        } else {
            Return {
                nullable: true,
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
    pub parameters: Parameters,
    pub ret: Return,
}

impl Function {
    pub fn parse(toml: &Value, object_name: &str) -> Option<Function> {
        let ident = match Ident::parse(toml, object_name, false) {
            Some(ident) => ident,
            None => {
                error!("No 'name' or 'pattern' given for function for object {}", object_name);
                return None
            }
        };
        let ignore = toml.lookup("ignore")
            .and_then(|val| val.as_bool())
            .unwrap_or(false);
        let parameters = Parameters::parse(toml.lookup("parameter"), object_name);
        let ret = Return::parse(toml.lookup("return"));

        Some(Function{
            ident: ident,
            ignore: ignore,
            parameters: parameters,
            ret: ret,
        })
    }

    pub fn matched_parameters<'a>(functions: &[&'a Function], parameter_name: &str) -> Vec<&'a Parameter> {
        let mut v = Vec::new();
        for f in functions {
            let pars = f.parameters.matched(parameter_name);
            //TODO: change to push_all
            for par in pars {
                v.push(par);
            }
        }
        v
    }
}

#[derive(Clone, Debug)]
pub struct Functions(Vec<Function>);

impl Functions {
    pub fn new() -> Functions {
        Functions(Vec::new())
    }

    pub fn parse(toml: Option<&Value>, object_name: &str) -> Functions {
        let mut v = Vec::new();
        if let Some(fns) = toml.and_then(|val| val.as_slice()) {
            for f in fns {
                if let Some(f) = Function::parse(f, object_name) {
                    v.push(f);
                }
            }
        }

        Functions(v)
    }

    pub fn matched(&self, function_name: &str) -> Vec<&Function> {
        self.0.iter().filter(|f| f.ident.is_match(function_name)).collect()
    }

    #[cfg(test)]
    fn vec(&self) -> &Vec<Function> {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml;

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
    fn function_parse_return_nullable_default1() {
        let toml = toml(r#"
name = "func1"
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ret.nullable, true);
    }
    #[test]
    fn function_parse_return_nullable_default2() {
        let toml = toml(r#"
name = "func1"
[return]
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ret.nullable, true);
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
[[parameter]]
name = "par3"
const = true
[[parameter]]
pattern = "par4"
const = true
"#);
        let f = Function::parse(&toml, "a").unwrap();
        let pars = f.parameters.vec();
        assert_eq!(pars.len(), 4);
        assert_eq!(pars[0].ident, Ident::Name("par1".into()));
        assert_eq!(pars[0].constant, false);
        assert_eq!(pars[1].ident, Ident::Name("par2".into()));
        assert_eq!(pars[1].constant, false);
        assert_eq!(pars[2].ident, Ident::Name("par3".into()));
        assert_eq!(pars[2].constant, true);
        if let Ident::Pattern(_) = pars[3].ident {
        } else {
            assert!(false, "Pattern don't parsed");
        }
        assert_eq!(pars[3].constant, true);
    }
    
    #[test]
    fn function_parse_return_nullable_false() {
        let toml = toml(r#"
name = "func1"
[return]
nullable = false
"#);
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ret.nullable, false);
    }

    #[test]
    fn functions_parse_empty_for_none() {
        let fns = Functions::parse(None, "a");
        assert!(fns.vec().is_empty());
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
        assert_eq!(fns.vec().len(), 3);
        assert_eq!(fns.vec()[0].ident, Ident::Name("func1".into()));
        assert_eq!(fns.vec()[1].ident, Ident::Name("func2".into()));
        if let Ident::Pattern(_) = fns.vec()[2].ident {
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
        assert_eq!(fns.vec().len(), 4);

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
        assert_eq!(fns.vec().len(), 2);
        let m = fns.matched("func");
        assert_eq!(m.len(), 2);

        assert_eq!(Function::matched_parameters(&m, "param").len(), 0);
        assert_eq!(Function::matched_parameters(&m, "par1").len(), 3);
        assert_eq!(Function::matched_parameters(&m, "par2").len(), 4);
        assert_eq!(Function::matched_parameters(&m, "par3").len(), 3);
        assert_eq!(Function::matched_parameters(&m, "par4").len(), 2);
    }
}
