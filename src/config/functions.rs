use std::{collections::HashSet, str::FromStr};

use log::error;
use toml::Value;

use super::{
    error::TomlHelper,
    gobjects::GStatus,
    ident::Ident,
    parameter_matchable::Functionlike,
    parsable::{Parsable, Parse},
    string_type::StringType,
};
use crate::{
    analysis::safety_assertion_mode::SafetyAssertionMode,
    codegen::Visibility,
    library::{Infallible, Mandatory, Nullable},
    version::Version,
};

#[derive(Clone, Debug)]
pub struct CallbackParameter {
    pub ident: Ident,
    pub nullable: Option<Nullable>,
}

pub type CallbackParameters = Vec<CallbackParameter>;

impl Parse for CallbackParameter {
    fn parse(toml: &Value, object_name: &str) -> Option<Self> {
        let ident = match Ident::parse(toml, object_name, "callback parameter") {
            Some(ident) => ident,
            None => {
                error!(
                    "No 'name' or 'pattern' given for parameter for object {}",
                    object_name
                );
                return None;
            }
        };
        toml.check_unwanted(&["nullable"], &format!("callback parameter {object_name}"));

        let nullable = toml
            .lookup("nullable")
            .and_then(Value::as_bool)
            .map(Nullable);

        Some(Self { ident, nullable })
    }
}

impl AsRef<Ident> for CallbackParameter {
    fn as_ref(&self) -> &Ident {
        &self.ident
    }
}

#[derive(Clone, Debug)]
pub struct Parameter {
    pub ident: Ident,
    // true - parameter don't changed in FFI function,
    // false(default) - parameter can be changed in FFI function
    pub constant: bool,
    pub move_: Option<bool>,
    pub nullable: Option<Nullable>,
    pub mandatory: Option<Mandatory>,
    pub infallible: Option<Infallible>,
    pub length_of: Option<String>,
    pub string_type: Option<StringType>,
    pub callback_parameters: CallbackParameters,
}

impl Parse for Parameter {
    fn parse(toml: &Value, object_name: &str) -> Option<Self> {
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
            &[
                "const",
                "nullable",
                "mandatory",
                "infallible",
                "length_of",
                "name",
                "move",
                "pattern",
                "string_type",
                "callback_parameter",
            ],
            &format!("function parameter {object_name}"),
        );

        let constant = toml
            .lookup("const")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let move_ = toml.lookup("move").and_then(Value::as_bool);
        let nullable = toml
            .lookup("nullable")
            .and_then(Value::as_bool)
            .map(Nullable);
        let mandatory = toml
            .lookup("mandatory")
            .and_then(Value::as_bool)
            .map(Mandatory);
        let infallible = toml
            .lookup("infallible")
            .and_then(Value::as_bool)
            .map(Infallible);
        let length_of = toml
            .lookup("length_of")
            .and_then(Value::as_str)
            .map(|s| if s == "return" { "" } else { s })
            .map(ToOwned::to_owned);
        let string_type = toml.lookup("string_type").and_then(Value::as_str);
        let string_type = match string_type {
            None => None,
            Some(val) => match StringType::from_str(val) {
                Ok(val) => Some(val),
                Err(error_str) => {
                    error!(
                        "Error: {} for parameter for object {}",
                        error_str, object_name
                    );
                    None
                }
            },
        };
        let callback_parameters =
            CallbackParameters::parse(toml.lookup("callback_parameter"), object_name);

        Some(Self {
            ident,
            constant,
            move_,
            nullable,
            mandatory,
            infallible,
            length_of,
            string_type,
            callback_parameters,
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
    pub mandatory: Option<Mandatory>,
    pub infallible: Option<Infallible>,
    pub bool_return_is_error: Option<String>,
    pub nullable_return_is_error: Option<String>,
    pub use_return_for_result: Option<bool>,
    pub string_type: Option<StringType>,
    pub type_name: Option<String>,
}

impl Return {
    pub fn parse(toml: Option<&Value>, object_name: &str) -> Self {
        if toml.is_none() {
            return Self {
                nullable: None,
                mandatory: None,
                infallible: None,
                bool_return_is_error: None,
                nullable_return_is_error: None,
                use_return_for_result: None,
                string_type: None,
                type_name: None,
            };
        }

        let v = toml.unwrap();
        v.check_unwanted(
            &[
                "nullable",
                "mandatory",
                "infallible",
                "bool_return_is_error",
                "nullable_return_is_error",
                "use_return_for_result",
                "string_type",
                "type",
            ],
            "return",
        );

        let nullable = v.lookup("nullable").and_then(Value::as_bool).map(Nullable);
        let mandatory = v
            .lookup("mandatory")
            .and_then(Value::as_bool)
            .map(Mandatory);
        let infallible = v
            .lookup("infallible")
            .and_then(Value::as_bool)
            .map(Infallible);
        let bool_return_is_error = v
            .lookup("bool_return_is_error")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let nullable_return_is_error = v
            .lookup("nullable_return_is_error")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let use_return_for_result = v.lookup("use_return_for_result").and_then(Value::as_bool);
        let string_type = v.lookup("string_type").and_then(Value::as_str);
        let string_type = match string_type {
            None => None,
            Some(v) => match StringType::from_str(v) {
                Ok(v) => Some(v),
                Err(error_str) => {
                    error!("Error: {} for return", error_str);
                    None
                }
            },
        };
        let type_name = v
            .lookup("type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        if string_type.is_some() && type_name.is_some() {
            error!(
                "\"string_type\" and \"type\" parameters can't be passed at the same time for \
                 object {}, only \"type\" will be applied in this case",
                object_name
            );
        }

        Self {
            nullable,
            mandatory,
            infallible,
            bool_return_is_error,
            nullable_return_is_error,
            use_return_for_result,
            string_type,
            type_name,
        }
    }
}

fn check_rename(rename: &Option<String>, object_name: &str, function_name: &Ident) -> bool {
    if let Some(rename) = rename {
        for c in &["\t", "\n", " "] {
            if rename.contains(c) {
                error!(
                    "Invalid 'rename' value given to {}::{}: forbidden character '{:?}'",
                    object_name, function_name, c
                );
                return false;
            }
        }
    }
    true
}

#[derive(Clone, Debug)]
pub struct Function {
    pub ident: Ident,
    pub status: GStatus,
    pub version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub parameters: Parameters,
    pub ret: Return,
    pub doc_hidden: bool,
    pub doc_ignore_parameters: HashSet<String>,
    pub disable_length_detect: bool,
    pub doc_trait_name: Option<String>,
    pub doc_struct_name: Option<String>,
    pub no_future: bool,
    pub unsafe_: bool,
    pub rename: Option<String>,
    pub visibility: Option<Visibility>,
    pub bypass_auto_rename: bool,
    pub is_constructor: Option<bool>,
    pub assertion: Option<SafetyAssertionMode>,
    pub generate_doc: bool,
}

impl Parse for Function {
    fn parse(toml: &Value, object_name: &str) -> Option<Self> {
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
                "manual",
                "version",
                "cfg_condition",
                "parameter",
                "return",
                "name",
                "doc_hidden",
                "doc_ignore_parameters",
                "disable_length_detect",
                "pattern",
                "doc_trait_name",
                "doc_struct_name",
                "no_future",
                "unsafe",
                "rename",
                "bypass_auto_rename",
                "constructor",
                "assertion",
                "visibility",
                "generate_doc",
            ],
            &format!("function {object_name}"),
        );

        let status = {
            if toml
                .lookup("ignore")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                GStatus::Ignore
            } else if toml
                .lookup("manual")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                GStatus::Manual
            } else {
                GStatus::Generate
            }
        };
        let version = toml
            .lookup("version")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok());
        let cfg_condition = toml
            .lookup("cfg_condition")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let parameters = Parameters::parse(toml.lookup("parameter"), object_name);
        let ret = Return::parse(toml.lookup("return"), object_name);
        let doc_hidden = toml
            .lookup("doc_hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let doc_ignore_parameters = toml
            .lookup_vec("doc_ignore_parameters", "Invalid doc_ignore_parameters")
            .map(|v| {
                v.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let disable_length_detect = toml
            .lookup("disable_length_detect")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let doc_trait_name = toml
            .lookup("doc_trait_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let doc_struct_name = toml
            .lookup("doc_struct_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let no_future = toml
            .lookup("no_future")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let unsafe_ = toml
            .lookup("unsafe")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let rename = toml
            .lookup("rename")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        if !check_rename(&rename, object_name, &ident) {
            return None;
        }
        let bypass_auto_rename = toml
            .lookup("bypass_auto_rename")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let is_constructor = toml.lookup("constructor").and_then(Value::as_bool);
        let assertion = toml
            .lookup("assertion")
            .and_then(Value::as_str)
            .map(|s| s.parse::<SafetyAssertionMode>())
            .transpose();
        if let Err(ref err) = assertion {
            error!("{}", err);
        }
        let assertion = assertion.ok().flatten();
        let visibility = toml
            .lookup("visibility")
            .and_then(Value::as_str)
            .map(std::str::FromStr::from_str)
            .transpose();
        if let Err(ref err) = visibility {
            error!("{}", err);
        }
        let visibility = visibility.ok().flatten();
        let generate_doc = toml
            .lookup("generate_doc")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        Some(Self {
            ident,
            status,
            version,
            cfg_condition,
            parameters,
            ret,
            doc_hidden,
            doc_ignore_parameters,
            disable_length_detect,
            doc_trait_name,
            doc_struct_name,
            no_future,
            unsafe_,
            rename,
            visibility,
            bypass_auto_rename,
            is_constructor,
            assertion,
            generate_doc,
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
    use super::{
        super::{
            ident::Ident,
            matchable::Matchable,
            parameter_matchable::ParameterMatchable,
            parsable::{Parsable, Parse},
        },
        *,
    };
    use crate::{library::Nullable, version::Version};

    fn functions_toml(input: &str) -> ::toml::Value {
        let mut value: ::toml::value::Table = ::toml::from_str(input).unwrap();
        value.remove("f").unwrap()
    }

    fn toml(input: &str) -> ::toml::Value {
        let value = ::toml::from_str(input);
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
        assert!(f.status.ignored());
    }

    #[test]
    fn function_parse_manual() {
        let toml = toml(
            r#"
name = "func1"
manual = true
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ident, Ident::Name("func1".into()));
        assert!(f.status.manual());
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
        assert!(f.status.need_generate());
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
        assert_eq!(f.version, Some(Version(3, 20, 0)));
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
        assert!(!pars[0].constant);
        assert_eq!(pars[0].nullable, None);
        assert_eq!(pars[1].ident, Ident::Name("par2".into()));
        assert!(!pars[1].constant);
        assert_eq!(pars[1].nullable, Some(Nullable(false)));
        assert_eq!(pars[2].ident, Ident::Name("par3".into()));
        assert!(pars[2].constant);
        assert_eq!(pars[2].nullable, Some(Nullable(true)));
        assert!(matches!(pars[3].ident, Ident::Pattern(_)));
        assert!(pars[3].constant);
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
    fn function_parse_generate_doc() {
        let r = toml(
            r#"
name = "prop"
generate_doc = false
"#,
        );
        let f = Function::parse(&r, "a").unwrap();
        assert!(!f.generate_doc);

        // Ensure that the default value is "true".
        let r = toml(
            r#"
name = "prop"
"#,
        );
        let f = Function::parse(&r, "a").unwrap();
        assert!(f.generate_doc);
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
        assert!(matches!(fns[2].ident, Ident::Pattern(_)));
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
        assert_eq!(fns.len(), 3);

        assert_eq!(fns.matched("func1").len(), 2);
        assert_eq!(fns.matched("func2").len(), 2);
        assert_eq!(fns.matched("func3").len(), 1);
        // "f1.5" is not a valid name
        assert_eq!(fns.matched("f1.5").len(), 0);
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

    #[test]
    fn functions_parse_rename() {
        let toml = toml(
            r#"
name = "func1"
rename = "another"
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.rename, Some("another".to_owned()));
    }

    #[test]
    fn functions_parse_rename_fail() {
        let toml = toml(
            r#"
name = "func1"
rename = "anoth er"
"#,
        );
        let f = Function::parse(&toml, "a");
        assert!(f.is_none());
    }

    #[test]
    fn function_bypass_auto_rename() {
        let toml = toml(
            r#"
name = "func1"
bypass_auto_rename = true
"#,
        );
        let f = Function::parse(&toml, "a").unwrap();
        assert_eq!(f.ident, Ident::Name("func1".into()));
        assert!(f.bypass_auto_rename);
    }

    #[test]
    fn parse_return_mandatory_default() {
        let toml = toml(
            r#"
name = "func1"
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        assert!(f.ret.mandatory.is_none());
    }

    #[test]
    fn parse_return_mandatory() {
        let toml = toml(
            r#"
name = "func1"
    [return]
    mandatory = true
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        assert_eq!(f.ret.mandatory, Some(Mandatory(true)));
    }

    #[test]
    fn parse_return_non_mandatory() {
        let toml = toml(
            r#"
name = "func1"
    [return]
    mandatory = false
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        assert_eq!(f.ret.mandatory, Some(Mandatory(false)));
    }

    #[test]
    fn parse_parameter_mandatory_default() {
        let toml = toml(
            r#"
name = "func1"
    [[parameter]]
    name = "param1"
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        let param1 = &f.parameters[0];
        assert!(param1.mandatory.is_none());
    }

    #[test]
    fn parse_parameter_mandatory() {
        let toml = toml(
            r#"
name = "func1"
    [[parameter]]
    name = "param1"
    mandatory = true
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        let param1 = &f.parameters[0];
        assert_eq!(param1.mandatory, Some(Mandatory(true)));
    }

    #[test]
    fn parse_parameter_non_mandatory() {
        let toml = toml(
            r#"
name = "func1"
    [[parameter]]
    name = "param1"
    mandatory = false
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        let param1 = &f.parameters[0];
        assert_eq!(param1.mandatory, Some(Mandatory(false)));
    }

    #[test]
    fn parse_return_infallible_default() {
        let toml = toml(
            r#"
name = "func1"
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        assert!(f.ret.infallible.is_none());
    }

    #[test]
    fn parse_return_infallible() {
        let toml = toml(
            r#"
name = "func1"
    [return]
    infallible = true
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        assert_eq!(f.ret.infallible, Some(Infallible(true)));
    }

    #[test]
    fn parse_return_faillible() {
        let toml = toml(
            r#"
name = "func1"
    [return]
    infallible = false
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        assert_eq!(f.ret.infallible, Some(Infallible(false)));
    }

    #[test]
    fn parse_parameter_infallible_default() {
        let toml = toml(
            r#"
name = "func1"
    [[parameter]]
    name = "param1"
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        let param1 = &f.parameters[0];
        assert!(param1.infallible.is_none());
    }

    #[test]
    fn parse_parameter_infallible() {
        let toml = toml(
            r#"
name = "func1"
    [[parameter]]
    name = "param1"
    infallible = true
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        let param1 = &f.parameters[0];
        assert_eq!(param1.infallible, Some(Infallible(true)));
    }

    #[test]
    fn parse_parameter_faillible() {
        let toml = toml(
            r#"
name = "func1"
    [[parameter]]
    name = "param1"
    infallible = false
"#,
        );
        let f = Function::parse(&toml, "a");
        let f = f.unwrap();
        let param1 = &f.parameters[0];
        assert_eq!(param1.infallible, Some(Infallible(false)));
    }
}
