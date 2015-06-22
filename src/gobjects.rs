use std::collections::HashMap;
use std::str::FromStr;
use toml::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GType {
    Enum,
    Interface,
    Widget,
    //TODO: Object, InitiallyUnowned,
}

impl FromStr for GType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "enum" => Ok(GType::Enum),
            "interface" => Ok(GType::Interface),
            "widget" => Ok(GType::Widget),
            _ => Err("Wrong object type".to_string())
        }
    }
}

#[derive(Clone, Debug, Eq,  PartialEq)]
pub enum GStatus {
    Manual,     //already generated
    Generate,
    Comment,
    Ignore,
}

impl FromStr for GStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(GStatus::Manual),
            "generate" => Ok(GStatus::Generate),
            "comment" => Ok(GStatus::Comment),
            "ignore" => Ok(GStatus::Ignore),
            _ => Err("Wrong object status".to_string())
        }
    }
}

/// Info about GObject descendant
#[derive(Debug)]
pub struct GObject {
    name: String,
    gtype: GType,
    status: GStatus,
}

pub type GObjects =  HashMap<String, GObject>;

pub fn parse_toml(toml_objects: &Value) -> GObjects {
    let mut objects = GObjects::new();
    for toml_object in toml_objects.as_slice().unwrap() {
        let gobject = parse_object(toml_object);
        objects.insert(gobject.name.clone(), gobject);
    }
    objects
}

fn parse_object(toml_object: &Value) -> GObject {
    let name = toml_object.lookup("name").unwrap_or_else(|| panic!("Object name not defined"))
        .as_str().unwrap().to_string();

    let gtype = match toml_object.lookup("type") {
        Some(value) => GType::from_str(value.as_str().unwrap()).unwrap_or(GType::Widget),
        None => GType::Widget,
    };
    let status = match toml_object.lookup("status") {
        Some(value) => GStatus::from_str(value.as_str().unwrap()).unwrap_or(GStatus::Ignore),
        None => GStatus::Ignore,
    };
    GObject { name: name, gtype: gtype, status: status }
}
