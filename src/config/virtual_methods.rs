use std::collections::HashSet;

use log::error;
use toml::Value;

use super::{
    error::TomlHelper,
    functions::{check_rename, Parameters, Return},
    gobjects::GStatus,
    ident::Ident,
    parsable::{Parsable, Parse},
};
use crate::version::Version;

#[derive(Clone, Debug)]
pub struct VirtualMethod {
    pub ident: Ident,
    pub status: GStatus,
    pub version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub parameters: Parameters,
    pub ret: Return,
    pub doc_hidden: bool,
    pub doc_ignore_parameters: HashSet<String>,
    pub doc_trait_name: Option<String>,
    pub unsafe_: bool,
    pub rename: Option<String>,
    pub bypass_auto_rename: bool,
    pub generate_doc: bool,
}

impl Parse for VirtualMethod {
    fn parse(toml: &Value, object_name: &str) -> Option<Self> {
        let ident = match Ident::parse(toml, object_name, "virtual_method") {
            Some(ident) => ident,
            None => {
                error!(
                    "No 'name' or 'pattern' given for virtual_method for object {}",
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
                "pattern",
                "doc_trait_name",
                "unsafe",
                "rename",
                "bypass_auto_rename",
                "generate_doc",
            ],
            &format!("virtual_method {object_name}"),
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
        let doc_trait_name = toml
            .lookup("doc_trait_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
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
            doc_trait_name,
            unsafe_,
            rename,
            bypass_auto_rename,
            generate_doc,
        })
    }
}

impl AsRef<Ident> for VirtualMethod {
    fn as_ref(&self) -> &Ident {
        &self.ident
    }
}

pub type VirtualMethods = Vec<VirtualMethod>;

#[cfg(test)]
mod tests {
    use super::{super::ident::Ident, *};

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
        let f = VirtualMethod::parse(&toml, "a").unwrap();
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
        let f = VirtualMethod::parse(&toml, "a").unwrap();
        assert_eq!(f.ident, Ident::Name("func1".into()));
        assert!(f.status.manual());
    }
}
