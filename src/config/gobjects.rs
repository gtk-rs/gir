use std::ascii::AsciiExt;
use std::collections::BTreeMap;
use std::str::FromStr;
use toml::Value;

use super::identables::Identables;
use super::functions::Functions;
use super::members::Members;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GStatus {
    Manual,     //already generated
    Generate,
    Comment,
    Ignore,
}

impl GStatus {
    pub fn ignored(&self) -> bool {
        self == &GStatus::Ignore
    }
    pub fn need_generate(&self) -> bool {
        self == &GStatus::Generate
    }
    pub fn normal(&self) -> bool {
        self == &GStatus::Generate || self == &GStatus::Manual
    }
}

impl Default for GStatus {
    fn default() -> GStatus { GStatus::Ignore }
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
    pub functions: Functions,
    pub members: Members,
    pub status: GStatus,
    pub module_name: Option<String>,
    pub cfg_condition: Option<String>,
}

impl Default for GObject {
    fn default() -> GObject {
        GObject {
            name: "Default".into(),
            functions: Functions::new(),
            members: Members::new(),
            status: Default::default(),
            module_name: None,
            cfg_condition: None,
        }
    }
}

//TODO: ?change to HashMap<String, GStatus>
pub type GObjects =  BTreeMap<String, GObject>;

pub fn parse_toml(toml_objects: &Value) -> GObjects {
    let mut objects = GObjects::new();
    for toml_object in toml_objects.as_slice().unwrap() {
        let gobject = parse_object(toml_object);
        objects.insert(gobject.name.clone(), gobject);
    }
    objects
}

fn parse_object(toml_object: &Value) -> GObject {
    let name: String = toml_object.lookup("name").expect("Object name not defined")
        .as_str().unwrap().into();

    let status = match toml_object.lookup("status") {
        Some(value) => GStatus::from_str(value.as_str().unwrap()).unwrap_or(Default::default()),
        None => Default::default(),
    };

    let functions = Functions::parse(toml_object.lookup("function"), &name);
    let members = Members::parse(toml_object.lookup("member"), &name);
    let module_name = toml_object.lookup("module_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let cfg_condition = toml_object.lookup("cfg_condition")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    GObject {
        name: name,
        functions: functions,
        members: members,
        status: status,
        module_name: module_name,
        cfg_condition: cfg_condition,
    }
}

pub fn parse_status_shorthands(objects: &mut GObjects, toml: &Value) {
    use self::GStatus::*;
    for &status in [Manual, Generate, Comment, Ignore].iter() {
        parse_status_shorthand(objects, status, toml);
    }
}

fn parse_status_shorthand(objects: &mut GObjects, status: GStatus, toml: &Value) {
    let name = format!("options.{:?}", status).to_ascii_lowercase();
    toml.lookup(&name).map(|a| a.as_slice().unwrap())
        .map(|a| for name_ in a.iter().map(|s| s.as_str().unwrap()) {
        match objects.get(name_) {
            None => {
                objects.insert(name_.into(), GObject {
                    name: name_.into(),
                    status: status,
                    .. Default::default()
                });
            },
            Some(_) => panic!("Bad name in {}: {} already defined", name, name_),
        }
    });
}
