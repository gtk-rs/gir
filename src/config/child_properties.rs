use toml::Value;
use super::parsable::Parse;

#[derive(Clone, Debug)]
pub struct ChildProperty {
    pub name: String,
    pub type_name: String,
}

impl Parse for ChildProperty {
    fn parse(toml: &Value, object_name: &str) -> Option<ChildProperty> {
        let name = toml.lookup("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        let name = if let Some(name) = name {
            name
        } else {
            error!("No child property name for `{}`", object_name);
            return None
        };

        let type_name = toml.lookup("type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        let type_name = if let Some(type_name) = type_name {
            type_name
        } else {
            error!("No type for child property `{}` for `{}`", name, object_name);
            return None
        };

        Some(ChildProperty {
            name: name,
            type_name: type_name,
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
    fn parse(toml_object: &Value, object_name: &str) -> Option<ChildProperties> {
        let child_name = toml_object.lookup("child_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        let child_type = toml_object.lookup("child_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        let mut properties: Vec<ChildProperty> = Vec::new();
        if let Some(configs) = toml_object.lookup("child_prop")
            .and_then(|val| val.as_slice()) {
            for config in configs {
                if let Some(item) = ChildProperty::parse(config, object_name) {
                    properties.push(item);
                }
            }
        }

        if !properties.is_empty() {
            Some(ChildProperties{
                child_name: child_name,
                child_type: child_type,
                properties: properties,
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
