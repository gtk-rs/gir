use std::collections::BTreeMap;
use std::str::FromStr;
use toml::Value;

use library;
use library::{Library, TypeId, MAIN_NAMESPACE};
use config::error::TomlHelper;
use config::parsable::{Parsable, Parse};
use super::child_properties::ChildProperties;
use super::derives::Derives;
use super::functions::Functions;
use super::constants::Constants;
use super::members::Members;
use super::properties::Properties;
use super::signals::{Signal, Signals};
use version::Version;
use analysis::{ref_mode, conversion_type};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GStatus {
    Manual, // already generated
    Generate,
    Comment,
    Ignore,
}

impl GStatus {
    pub fn ignored(self) -> bool {
        self == GStatus::Ignore
    }
    pub fn need_generate(self) -> bool {
        self == GStatus::Generate
    }
    pub fn normal(self) -> bool {
        self == GStatus::Generate || self == GStatus::Manual
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
    pub virtual_methods: Functions,
    pub constants: Constants,
    pub signals: Signals,
    pub members: Members,
    pub properties: Properties,
    pub derives: Option<Derives>,
    pub status: GStatus,
    pub module_name: Option<String>,
    pub version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub type_id: Option<TypeId>,
    pub generate_trait: bool,
    pub trait_name: Option<String>,
    pub subclass_impl_trait_name: Option<String>,
    pub subclass_base_trait_name: Option<String>,
    pub child_properties: Option<ChildProperties>,
    pub concurrency: library::Concurrency,
    pub ref_mode: Option<ref_mode::RefMode>,
    pub must_use: bool,
    pub conversion_type: Option<conversion_type::ConversionType>,
    pub use_boxed_functions: bool,
    pub generate_display_trait: bool,
    pub subclassing: bool,
}

impl Default for GObject {
    fn default() -> GObject {
        GObject {
            name: "Default".into(),
            functions: Functions::new(),
            virtual_methods: Functions::new(),
            constants: Constants::new(),
            signals: Signals::new(),
            members: Members::new(),
            properties: Properties::new(),
            derives: None,
            status: Default::default(),
            module_name: None,
            version: None,
            cfg_condition: None,
            type_id: None,
            generate_trait: true,
            trait_name: None,
            subclass_impl_trait_name: None,
            subclass_base_trait_name: None,
            child_properties: None,
            concurrency: Default::default(),
            ref_mode: None,
            must_use: false,
            conversion_type: None,
            use_boxed_functions: false,
            generate_display_trait: true,
            subclassing: false,
        }
    }
}

//TODO: ?change to HashMap<String, GStatus>
pub type GObjects = BTreeMap<String, GObject>;

pub fn parse_toml(
    toml_objects: &Value,
    concurrency: library::Concurrency,
    generate_display_trait: bool,
) -> GObjects {
    let mut objects = GObjects::new();
    for toml_object in toml_objects.as_array().unwrap() {
        let gobject = parse_object(toml_object, concurrency, generate_display_trait);
        objects.insert(gobject.name.clone(), gobject);
    }
    objects
}

fn ref_mode_from_str(ref_mode: &str) -> Option<ref_mode::RefMode> {
    use analysis::ref_mode::RefMode::*;

    match ref_mode {
        "none" => Some(None),
        "ref" => Some(ByRef),
        "ref-mut" => Some(ByRefMut),
        "ref-immut" => Some(ByRefImmut),
        "ref-fake" => Some(ByRefFake),
        _ => Option::None,
    }
}

fn conversion_type_from_str(conversion_type: &str) -> Option<conversion_type::ConversionType> {
    use analysis::conversion_type::ConversionType::*;

    match conversion_type {
        "direct" => Some(Direct),
        "scalar" => Some(Scalar),
        "pointer" => Some(Pointer),
        "borrow" => Some(Borrow),
        "unknown" => Some(Unknown),
        _ => None,
    }
}

fn parse_object(
    toml_object: &Value,
    concurrency: library::Concurrency,
    default_generate_display_trait: bool,
) -> GObject {
    let name: String = toml_object
        .lookup("name")
        .expect("Object name not defined")
        .as_str()
        .unwrap()
        .into();
    // Also checks for ChildProperties
    toml_object.check_unwanted(
        &[
            "name",
            "status",
            "function",
            "virtual_method",
            "constant",
            "signal",
            "member",
            "property",
            "derive",
            "module_name",
            "version",
            "concurrency",
            "ref_mode",
            "conversion_type",
            "child_prop",
            "child_name",
            "child_type",
            "trait",
            "trait_name",
            "subclass_impl_trait_name",
            "subclass_base_trait_name",
            "cfg_condition",
            "must_use",
            "use_boxed_functions",
            "generate_display_trait",
            "subclassing",
        ],
        &format!("object {}", name),
    );

    let status = match toml_object.lookup("status") {
        Some(value) => {
            GStatus::from_str(value.as_str().unwrap()).unwrap_or_else(|_| Default::default())
        }
        None => Default::default(),
    };

    let constants = Constants::parse(toml_object.lookup("constant"), &name);
    let functions = Functions::parse(toml_object.lookup("function"), &name);
    let virtual_methods = Functions::parse(toml_object.lookup("virtual_methods"), &name);
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
    let derives = if let Some(derives) = toml_object.lookup("derive") {
        Some(Derives::parse(Some(derives), &name))
    } else {
        None
    };
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
    let subclass_impl_trait_name = toml_object
        .lookup("subclass_impl_trait_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let subclass_base_trait_name = toml_object
        .lookup("subclass_base_trait_name")
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
    let conversion_type = toml_object
        .lookup("conversion_type")
        .and_then(|v| v.as_str())
        .and_then(conversion_type_from_str);
    let child_properties = ChildProperties::parse(toml_object, &name);
    let must_use = toml_object
        .lookup("must_use")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let use_boxed_functions = toml_object
        .lookup("use_boxed_functions")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let generate_display_trait = toml_object
        .lookup("generate_display_trait")
        .and_then(|v| v.as_bool())
        .unwrap_or(default_generate_display_trait);
    let subclassing = toml_object
        .lookup("subclassing")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if status != GStatus::Manual && ref_mode.is_some() {
        warn!("ref_mode configuration used for non-manual object {}", name);
    }

    if status != GStatus::Manual && conversion_type.is_some() {
        warn!("conversion_type configuration used for non-manual object {}", name);
    }

    GObject {
        name,
        functions,
        virtual_methods,
        constants,
        signals,
        members,
        properties,
        derives,
        status,
        module_name,
        version,
        cfg_condition,
        type_id: None,
        generate_trait,
        trait_name,
        subclass_impl_trait_name,
        subclass_base_trait_name,
        child_properties,
        concurrency,
        ref_mode,
        must_use,
        conversion_type,
        use_boxed_functions,
        generate_display_trait,
        subclassing,
    }
}

pub fn parse_status_shorthands(
    objects: &mut GObjects,
    toml: &Value,
    concurrency: library::Concurrency,
    generate_display_trait: bool,
) {
    use self::GStatus::*;
    for &status in &[Manual, Generate, Comment, Ignore] {
        parse_status_shorthand(objects, status, toml, concurrency, generate_display_trait);
    }
}

fn parse_status_shorthand(
    objects: &mut GObjects,
    status: GStatus,
    toml: &Value,
    concurrency: library::Concurrency,
    generate_display_trait: bool,
) {
    let name = format!("options.{:?}", status).to_ascii_lowercase();
    if let Some(a) = toml.lookup(&name).map(|a| a.as_array().unwrap()) {
        for name_ in a.iter().map(|s| s.as_str().unwrap()) {
            match objects.get(name_) {
                None => {
                    objects.insert(
                        name_.into(),
                        GObject {
                            name: name_.into(),
                            status,
                            concurrency,
                            generate_display_trait,
                            ..Default::default()
                        },
                    );
                }
                Some(_) => panic!("Bad name in {}: {} already defined", name, name_),
            }
        }
    }
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
