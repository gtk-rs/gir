use std::ascii::AsciiExt;
use std::collections::BTreeMap;
use std::str::FromStr;
use toml::Value;

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
    pub non_nullable_overrides: Vec<String>, // sorted
    pub ignored_functions: Vec<String>, // sorted
    pub status: GStatus,
}

impl Default for GObject {
    fn default() -> GObject {
        GObject {
            name: "Default".into(),
            non_nullable_overrides: Vec::new(),
            ignored_functions: Vec::new(),
            status: Default::default(),
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
    let name = toml_object.lookup("name").expect("Object name not defined")
        .as_str().unwrap().into();

    let status = match toml_object.lookup("status") {
        Some(value) => GStatus::from_str(value.as_str().unwrap()).unwrap_or(Default::default()),
        None => Default::default(),
    };

    let mut non_nullable_overrides = Vec::new();
    if let Some(fn_names) = toml_object.lookup("non_nullable").and_then(|o| o.as_slice()) {
        non_nullable_overrides = fn_names.iter()
            .filter_map(|fn_name| fn_name.as_str().map(String::from))
            .collect();
        non_nullable_overrides.sort();
    }

    let mut ignored_functions = Vec::new();
    if let Some(fn_names) = toml_object.lookup("ignored_functions").and_then(|o| o.as_slice()) {
        ignored_functions = fn_names.iter()
            .filter_map(|fn_name| fn_name.as_str().map(String::from))
            .collect();
        ignored_functions.sort();
    }

    GObject {
        name: name,
        non_nullable_overrides: non_nullable_overrides,
        ignored_functions: ignored_functions,
        status: status
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
