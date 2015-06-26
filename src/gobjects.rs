use std::collections::HashMap;
use std::str::FromStr;
use toml::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GType {
    Enum,
    Interface,
    Widget,
    Unknown,
    //TODO: Object, InitiallyUnowned,
}

impl FromStr for GType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "enum" => Ok(GType::Enum),
            "interface" => Ok(GType::Interface),
            "widget" => Ok(GType::Widget),
            _ => Err("Wrong object type".into())
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
            _ => Err("Wrong object status".into())
        }
    }
}

/// Info about GObject descendant
#[derive(Clone, Debug)]
pub struct GObject {
    pub name: String,
    pub gtype: GType,
    pub status: GStatus,
    pub last_parent: bool,
}

impl Default for GObject {
    fn default() -> GObject {
        GObject {
            name: "Default".into(),
            gtype: GType::Unknown,
            status: GStatus::Ignore,
            last_parent: false,
        }
    }
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
        .as_str().unwrap().into();

    let gtype = match toml_object.lookup("type") {
        Some(value) => GType::from_str(value.as_str().unwrap()).unwrap_or(GType::Widget),
        None => GType::Widget,
    };
    let status = match toml_object.lookup("status") {
        Some(value) => GStatus::from_str(value.as_str().unwrap()).unwrap_or(GStatus::Ignore),
        None => GStatus::Ignore,
    };
    let last_parent = match toml_object.lookup("last_parent") {
        Some(&Value::Boolean(b)) => b,
        _ => false,
    };
    GObject { name: name, gtype: gtype, status: status, last_parent: last_parent }
}
