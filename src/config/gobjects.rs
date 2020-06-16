use super::{
    child_properties::ChildProperties,
    constants::Constants,
    derives::Derives,
    functions::Functions,
    members::Members,
    properties::Properties,
    signals::{Signal, Signals},
};
use crate::{
    analysis::{conversion_type, ref_mode},
    config::{
        error::TomlHelper,
        parsable::{Parsable, Parse},
    },
    library::{self, Library, TypeId, MAIN_NAMESPACE},
    version::Version,
};
use log::warn;
use std::{collections::BTreeMap, str::FromStr};
use toml::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GStatus {
    Manual,
    Generate,
    Ignore,
}

impl GStatus {
    pub fn ignored(self) -> bool {
        self == GStatus::Ignore
    }
    pub fn need_generate(self) -> bool {
        self == GStatus::Generate
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
    pub final_type: Option<bool>,
    pub trait_name: Option<String>,
    pub child_properties: Option<ChildProperties>,
    pub concurrency: library::Concurrency,
    pub ref_mode: Option<ref_mode::RefMode>,
    pub must_use: bool,
    pub conversion_type: Option<conversion_type::ConversionType>,
    pub use_boxed_functions: bool,
    pub generate_display_trait: bool,
    pub manual_traits: Vec<String>,
    pub align: Option<u32>,
    pub generate_builder: bool,
    pub ignore_builder: bool,
    pub builder_postprocess: Option<String>,
    pub init_function_expression: Option<String>,
    pub clear_function_expression: Option<String>,
}

impl Default for GObject {
    fn default() -> GObject {
        GObject {
            name: "Default".into(),
            functions: Functions::new(),
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
            final_type: None,
            trait_name: None,
            child_properties: None,
            concurrency: Default::default(),
            ref_mode: None,
            must_use: false,
            conversion_type: None,
            use_boxed_functions: false,
            generate_display_trait: true,
            manual_traits: Vec::default(),
            align: None,
            generate_builder: false,
            ignore_builder: false,
            builder_postprocess: None,
            init_function_expression: None,
            clear_function_expression: None,
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
    use crate::analysis::ref_mode::RefMode::*;

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
    use crate::analysis::conversion_type::ConversionType::*;

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
            "final_type",
            "trait",
            "trait_name",
            "cfg_condition",
            "must_use",
            "use_boxed_functions",
            "generate_display_trait",
            "manual_traits",
            "align",
            "generate_builder",
            "ignore_builder",
            "builder_postprocess",
            "init_function_expression",
            "clear_function_expression",
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
    let signals = {
        let mut v = Vec::new();
        if let Some(configs) = toml_object.lookup("signal").and_then(Value::as_array) {
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
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let version = toml_object
        .lookup("version")
        .and_then(Value::as_str)
        .and_then(|s| s.parse().ok());
    let cfg_condition = toml_object
        .lookup("cfg_condition")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let generate_trait = toml_object.lookup("trait").and_then(Value::as_bool);
    let final_type = toml_object
        .lookup("final_type")
        .and_then(Value::as_bool)
        .or_else(|| generate_trait.map(|t| !t));
    let trait_name = toml_object
        .lookup("trait_name")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let concurrency = toml_object
        .lookup("concurrency")
        .and_then(Value::as_str)
        .and_then(|v| v.parse().ok())
        .unwrap_or(concurrency);
    let ref_mode = toml_object
        .lookup("ref_mode")
        .and_then(Value::as_str)
        .and_then(ref_mode_from_str);
    let conversion_type = toml_object
        .lookup("conversion_type")
        .and_then(Value::as_str)
        .and_then(conversion_type_from_str);
    let child_properties = ChildProperties::parse(toml_object, &name);
    let must_use = toml_object
        .lookup("must_use")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let use_boxed_functions = toml_object
        .lookup("use_boxed_functions")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let generate_display_trait = toml_object
        .lookup("generate_display_trait")
        .and_then(Value::as_bool)
        .unwrap_or(default_generate_display_trait);
    let manual_traits = toml_object
        .lookup_vec("manual_traits", "IGNORED ERROR")
        .map(|v| {
            v.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|_| Vec::new());
    let align = toml_object
        .lookup("align")
        .and_then(Value::as_integer)
        .and_then(|v| {
            if v.count_ones() != 1 || v > i64::from(u32::max_value()) || v < 0 {
                warn!(
                    "`align` configuration must be a power of two of type u32, found {}",
                    v
                );
                None
            } else {
                Some(v as u32)
            }
        });
    let generate_builder = toml_object
        .lookup("generate_builder")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let ignore_builder = toml_object
        .lookup("ignore_builder")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let builder_postprocess = toml_object
        .lookup("builder_postprocess")
        .and_then(Value::as_str)
        .map(String::from);
    let init_function_expression = toml_object
        .lookup("init_function_expression")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let clear_function_expression = toml_object
        .lookup("clear_function_expression")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    if (init_function_expression.is_some() && clear_function_expression.is_none())
        || (init_function_expression.is_none() && clear_function_expression.is_some())
    {
        panic!(
            "`init_function_expression` and `clear_function_expression` both have to be provided"
        );
    }

    if status != GStatus::Manual && ref_mode.is_some() {
        warn!("ref_mode configuration used for non-manual object {}", name);
    }

    if status != GStatus::Manual && conversion_type.is_some() {
        warn!(
            "conversion_type configuration used for non-manual object {}",
            name
        );
    }

    if generate_trait.is_some() {
        warn!(
            "`trait` configuration is deprecated and replaced by `final_type` for object {}",
            name
        );
    }

    GObject {
        name,
        functions,
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
        final_type,
        trait_name,
        child_properties,
        concurrency,
        ref_mode,
        must_use,
        conversion_type,
        use_boxed_functions,
        generate_display_trait,
        manual_traits,
        align,
        generate_builder,
        builder_postprocess,
        init_function_expression,
        clear_function_expression,
        ignore_builder,
    }
}

pub fn parse_status_shorthands(
    objects: &mut GObjects,
    toml: &Value,
    concurrency: library::Concurrency,
    generate_display_trait: bool,
) {
    use self::GStatus::*;
    for &status in &[Manual, Generate, Ignore] {
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
    let option_name = format!("options.{:?}", status).to_ascii_lowercase();
    if let Some(a) = toml.lookup(&option_name).map(|a| a.as_array().unwrap()) {
        for name in a.iter().map(|s| s.as_str().unwrap()) {
            match objects.get(name) {
                None => {
                    objects.insert(
                        name.into(),
                        GObject {
                            name: name.into(),
                            status,
                            concurrency,
                            generate_display_trait,
                            ..Default::default()
                        },
                    );
                }
                Some(_) => panic!("Bad name in {}: {} already defined", option_name, name),
            }
        }
    }
}

pub fn parse_builders(objects: &mut GObjects, toml: &Value) {
    let builder_suffix = "Builder";
    let option_name = "options.builders";
    if let Some(a) = toml.lookup(option_name).map(|a| a.as_array().unwrap()) {
        for name in a.iter().map(|s| s.as_str().unwrap()) {
            // Support both object name and builder name
            let obj_name = if name.ends_with(builder_suffix) {
                &name[..name.len() - builder_suffix.len()]
            } else {
                name
            };
            match objects.get_mut(obj_name) {
                Some(obj) => obj.generate_builder = true,
                None => panic!("Bad name in {}: object {} not defined", option_name, name),
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
