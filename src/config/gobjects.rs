use std::ascii::AsciiExt;
use std::collections::BTreeMap;
use std::str::FromStr;
use toml::Value;

use library::{Library, TypeId};
use config::error::TomlHelper;
use config::parsable::{Parsable, Parse};
use super::child_properties::ChildProperties;
use super::functions::Functions;
use super::members::Members;
use super::properties::Properties;
use super::signals::Signals;
use version::Version;

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

/// Info about `GObject` descendant
#[derive(Clone, Debug)]
pub struct GObject {
    pub name: String,
    pub functions: Functions,
    pub signals: Signals,
    pub members: Members,
    pub properties: Properties,
    pub status: GStatus,
    pub module_name: Option<String>,
    pub version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub type_id: Option<TypeId>,
    pub force_trait: bool,
    pub child_properties: Option<ChildProperties>,
}

impl Default for GObject {
    fn default() -> GObject {
        GObject {
            name: "Default".into(),
            functions: Functions::new(),
            signals: Signals::new(),
            members: Members::new(),
            properties: Properties::new(),
            status: Default::default(),
            module_name: None,
            version: None,
            cfg_condition: None,
            type_id: None,
            force_trait: false,
            child_properties: None,
        }
    }
}

//TODO: ?change to HashMap<String, GStatus>
pub type GObjects =  BTreeMap<String, GObject>;

pub fn parse_toml(toml_objects: &Value) -> GObjects {
    let mut objects = GObjects::new();
    for toml_object in toml_objects.as_array().unwrap() {
        let gobject = parse_object(toml_object);
        objects.insert(gobject.name.clone(), gobject);
    }
    objects
}

fn parse_object(toml_object: &Value) -> GObject {
    let name: String = toml_object.lookup("name").expect("Object name not defined")
        .as_str().unwrap().into();

    let status = match toml_object.lookup("status") {
        Some(value) => GStatus::from_str(value.as_str().unwrap())
            .unwrap_or_else(|_| Default::default()),
        None => Default::default(),
    };

    let functions = Functions::parse(toml_object.lookup("function"), &name);
    let signals = Signals::parse(toml_object.lookup("signal"), &name);
    let members = Members::parse(toml_object.lookup("member"), &name);
    let properties = Properties::parse(toml_object.lookup("property"), &name);
    let module_name = toml_object.lookup("module_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let version = toml_object.lookup("version")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok());
    let cfg_condition = toml_object.lookup("cfg_condition")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let force_trait = toml_object.lookup("trait")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let child_properties = ChildProperties::parse(toml_object, &name);

    GObject {
        name: name,
        functions: functions,
        signals: signals,
        members: members,
        properties: properties,
        status: status,
        module_name: module_name,
        version: version,
        cfg_condition: cfg_condition,
        type_id: None,
        force_trait: force_trait,
        child_properties: child_properties,
    }
}

pub fn parse_status_shorthands(objects: &mut GObjects, toml: &Value) {
    use self::GStatus::*;
    for &status in &[Manual, Generate, Comment, Ignore] {
        parse_status_shorthand(objects, status, toml);
    }
}

fn parse_status_shorthand(objects: &mut GObjects, status: GStatus, toml: &Value) {
    let name = format!("options.{:?}", status).to_ascii_lowercase();
    toml.lookup(&name).map(|a| a.as_array().unwrap())
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

pub fn resolve_type_ids(objects: &mut GObjects, library: &Library) {
    for (name, object) in objects.iter_mut() {
        let type_id = library.find_type(0, name);
        if type_id.is_none() {
            warn!("Configured object `{}` missing from the library", name);
        }
        object.type_id = type_id;
    }
}
