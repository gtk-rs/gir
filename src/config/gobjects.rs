use std::ascii::AsciiExt;
use std::collections::BTreeMap;
use std::str::FromStr;
use toml::Value;

use library;
use library::{Library, TypeId, MAIN_NAMESPACE};
use config::error::TomlHelper;
use config::parsable::{Parsable, Parse};
use super::child_properties::ChildProperties;
use super::functions::Functions;
use super::members::Members;
use super::properties::Properties;
use super::signals::{Signals, Signal};
use version::Version;
use analysis::ref_mode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GStatus {
    Manual, // already generated
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
    fn default() -> GStatus {
        GStatus::Ignore
    }
}

impl FromStr for GStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "manual" => Ok(GStatus::Manual),
            "generate" => Ok(GStatus::Generate),
            "comment" => Ok(GStatus::Comment),
            "ignore" => Ok(GStatus::Ignore),
            e => Err(format!("Wrong object status: \"{}\"", e)),
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
    pub generate_trait: bool,
    pub trait_name: Option<String>,
    pub child_properties: Option<ChildProperties>,
    pub concurrency: library::Concurrency,
    pub ref_mode: Option<ref_mode::RefMode>,
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
            generate_trait: true,
            trait_name: None,
            child_properties: None,
            concurrency: Default::default(),
            ref_mode: None,
        }
    }
}

//TODO: ?change to HashMap<String, GStatus>
pub type GObjects = BTreeMap<String, GObject>;

pub fn parse_toml(toml_objects: &Value, concurrency: library::Concurrency) -> GObjects {
    let mut objects = GObjects::new();
    for toml_object in toml_objects.as_array().unwrap() {
        let gobject = parse_object(toml_object, concurrency);
        objects.insert(gobject.name.clone(), gobject);
    }
    objects
}

fn ref_mode_from_str(ref_mode: &str) -> Option<ref_mode::RefMode> {
    match ref_mode {
        "none" => Some(ref_mode::RefMode::None),
        "ref" => Some(ref_mode::RefMode::ByRef),
        "ref-mut" => Some(ref_mode::RefMode::ByRefMut),
        "ref-immut" => Some(ref_mode::RefMode::ByRefImmut),
        "ref-fake" => Some(ref_mode::RefMode::ByRefFake),
        _ => None,
    }
}

fn parse_object(toml_object: &Value, concurrency: library::Concurrency) -> GObject {
    let name: String = toml_object
        .lookup("name")
        .expect("Object name not defined")
        .as_str()
        .unwrap()
        .into();
    // Also checks for ChildProperties
    toml_object.check_unwanted(&["name", "status", "function", "signal", "member", "property",
                                 "module_name", "version", "concurrency", "ref_mode", "child_prop",
                                 "child_name", "child_type"],
                               &format!("object {}", name));

    let status = match toml_object.lookup("status") {
        Some(value) => {
            GStatus::from_str(value.as_str().unwrap()).unwrap_or_else(|_| Default::default())
        }
        None => Default::default(),
    };

    let functions = Functions::parse(toml_object.lookup("function"), &name);
    let signals = {
        let mut v = Vec::new();
        if let Some(configs) = toml_object.lookup("signal").and_then(|val| val.as_array()) {
            for config in configs {
                if let Some(item) = Signal::parse(config, &name, concurrency) {
                    v.push(item);
                }
            }
        }

        v
    };
    let members = Members::parse(toml_object.lookup("member"), &name);
    let properties = Properties::parse(toml_object.lookup("property"), &name);
    let module_name = toml_object
        .lookup("module_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let version = toml_object
        .lookup("version")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok());
    let cfg_condition = toml_object
        .lookup("cfg_condition")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let generate_trait = toml_object
        .lookup("trait")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let trait_name = toml_object
        .lookup("trait_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let concurrency = toml_object
        .lookup("concurrency")
        .and_then(|v| v.as_str())
        .and_then(|v| v.parse().ok())
        .unwrap_or(concurrency);
    let ref_mode = toml_object
        .lookup("ref_mode")
        .and_then(|v| v.as_str())
        .and_then(ref_mode_from_str);
    let child_properties = ChildProperties::parse(toml_object, &name);

    if status != GStatus::Manual {
        if ref_mode != None {
            warn!("ref_mode configuration used for non-manual object {}", name);
        }
    }

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
        generate_trait: generate_trait,
        trait_name: trait_name,
        child_properties: child_properties,
        concurrency: concurrency,
        ref_mode: ref_mode,
    }
}

pub fn parse_status_shorthands(
    objects: &mut GObjects,
    toml: &Value,
    concurrency: library::Concurrency,
) {
    use self::GStatus::*;
    for &status in &[Manual, Generate, Comment, Ignore] {
        parse_status_shorthand(objects, status, toml, concurrency);
    }
}

fn parse_status_shorthand(
    objects: &mut GObjects,
    status: GStatus,
    toml: &Value,
    concurrency: library::Concurrency,
) {
    let name = format!("options.{:?}", status).to_ascii_lowercase();
    toml.lookup(&name).map(|a| a.as_array().unwrap()).map(
        |a| for name_ in a.iter().map(|s| s.as_str().unwrap()) {
            match objects.get(name_) {
                None => {
                    objects.insert(
                        name_.into(),
                        GObject {
                            name: name_.into(),
                            status: status,
                            concurrency: concurrency,
                            ..Default::default()
                        },
                    );
                }
                Some(_) => panic!("Bad name in {}: {} already defined", name, name_),
            }
        },
    );
}

pub fn resolve_type_ids(objects: &mut GObjects, library: &Library) {
    let ns = library.namespace(MAIN_NAMESPACE);
    let global_functions_name = format!("{}.*", ns.name);

    for (name, object) in objects.iter_mut() {
        let type_id = library.find_type(0, name);
        if type_id.is_none() && name != &global_functions_name {
            warn!("Configured object `{}` missing from the library", name);
        }
        object.type_id = type_id;
    }
}
