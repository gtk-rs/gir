use log::error;
use toml::Value;

use super::{error::TomlHelper, parsable::Parse};

#[derive(Clone, Debug)]
pub struct ChildProperty {
    pub name: String,
    pub rename_getter: Option<String>,
    pub type_name: String,
    pub doc_hidden: bool,
    pub generate_doc: bool,
}

impl Parse for ChildProperty {
    fn parse(toml: &Value, object_name: &str) -> Option<Self> {
        let name = toml
            .lookup("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let name = if let Some(name) = name {
            name
        } else {
            error!("No child property name for `{}`", object_name);
            return None;
        };

        toml.check_unwanted(
            &["name", "type", "doc_hidden", "rename_getter"],
            &format!("child property {object_name}"),
        );

        let type_name = toml
            .lookup("type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let type_name = if let Some(type_name) = type_name {
            type_name
        } else {
            error!(
                "No type for child property `{}` for `{}`",
                name, object_name
            );
            return None;
        };
        let doc_hidden = toml
            .lookup("doc_hidden")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let rename_getter = toml
            .lookup("rename_getter")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let generate_doc = toml
            .lookup("generate_doc")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        Some(Self {
            name,
            rename_getter,
            type_name,
            doc_hidden,
            generate_doc,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ChildProperties {
    pub child_name: Option<String>,
    pub child_type: Option<String>,
    pub properties: Vec<ChildProperty>,
}

impl Parse for ChildProperties {
    fn parse(toml_object: &Value, object_name: &str) -> Option<Self> {
        let child_name = toml_object
            .lookup("child_name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let child_type = toml_object
            .lookup("child_type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let mut properties: Vec<ChildProperty> = Vec::new();
        if let Some(configs) = toml_object.lookup("child_prop").and_then(Value::as_array) {
            for config in configs {
                if let Some(item) = ChildProperty::parse(config, object_name) {
                    properties.push(item);
                }
            }
        }

        if !properties.is_empty() {
            Some(Self {
                child_name,
                child_type,
                properties,
            })
        } else {
            if child_name.is_some() {
                error!("`{}` has child_name but no child_prop's", object_name);
            }
            if child_type.is_some() {
                error!("`{}` has child_type but no child_prop's", object_name);
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{super::parsable::Parse, *};

    fn toml(input: &str) -> ::toml::Value {
        let value = ::toml::from_str(input);
        assert!(value.is_ok());
        value.unwrap()
    }

    #[test]
    fn child_property_parse() {
        let toml = toml(
            r#"
name = "prop"
type = "prop_type"
"#,
        );
        let child = ChildProperty::parse(&toml, "a").unwrap();
        assert_eq!("prop", child.name);
        assert_eq!("prop_type", child.type_name);
    }

    #[test]
    fn child_property_parse_generate_doc() {
        let r = toml(
            r#"
name = "prop"
type = "prop_type"
generate_doc = false
"#,
        );
        let child = ChildProperty::parse(&r, "a").unwrap();
        assert!(!child.generate_doc);

        // Ensure that the default value is "true".
        let r = toml(
            r#"
name = "prop"
type = "prop_type"
"#,
        );
        let child = ChildProperty::parse(&r, "a").unwrap();
        assert!(child.generate_doc);
    }

    #[test]
    fn child_property_parse_not_all() {
        let tml = toml(
            r#"
name = "prop"
"#,
        );
        assert!(ChildProperty::parse(&tml, "a").is_none());

        let tml = toml(
            r#"
type_name = "prop_type"
"#,
        );
        assert!(ChildProperty::parse(&tml, "a").is_none());
    }

    #[test]
    fn child_properties_parse() {
        let toml = toml(
            r#"
child_name = "child_name"
child_type = "child_type"
[[child_prop]]
name = "prop"
type = "prop_type"
[[child_prop]]
name = "prop2"
type = "prop_type2"
"#,
        );
        let props = ChildProperties::parse(&toml, "a").unwrap();
        assert_eq!(Some("child_name".into()), props.child_name);
        assert_eq!(Some("child_type".into()), props.child_type);
        assert_eq!(2, props.properties.len());
        assert_eq!("prop", props.properties[0].name);
        assert_eq!("prop_type", props.properties[0].type_name);
        assert_eq!("prop2", props.properties[1].name);
        assert_eq!("prop_type2", props.properties[1].type_name);
    }

    #[test]
    fn child_property_no_parse_without_children() {
        let toml = toml(
            r#"
child_name = "child_name"
child_type = "child_type"
"#,
        );
        let props = ChildProperties::parse(&toml, "a");
        assert!(props.is_none());
    }

    #[test]
    fn child_properties_parse_without_child_type_name() {
        let toml = toml(
            r#"
[[child_prop]]
name = "prop"
type = "prop_type"
"#,
        );
        let props = ChildProperties::parse(&toml, "a").unwrap();
        assert_eq!(None, props.child_name);
        assert_eq!(None, props.child_type);
        assert_eq!(1, props.properties.len());
    }

    #[test]
    fn child_properties_parse_without_child_type() {
        let toml = toml(
            r#"
child_name = "child_name"
[[child_prop]]
name = "prop"
type = "prop_type"
"#,
        );
        let props = ChildProperties::parse(&toml, "a").unwrap();
        assert_eq!(Some("child_name".into()), props.child_name);
        assert_eq!(None, props.child_type);
        assert_eq!(1, props.properties.len());
    }

    #[test]
    fn child_properties_parse_without_child_name() {
        let toml = toml(
            r#"
child_type = "child_type"
[[child_prop]]
name = "prop"
type = "prop_type"
"#,
        );
        let props = ChildProperties::parse(&toml, "a").unwrap();
        assert_eq!(None, props.child_name);
        assert_eq!(Some("child_type".into()), props.child_type);
        assert_eq!(1, props.properties.len());
    }
}
