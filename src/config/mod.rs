mod child_properties;
#[allow(clippy::module_inception)]
pub mod config;
pub mod constants;
pub mod derives;
pub mod error;
mod external_libraries;
pub mod functions;
pub mod gobjects;
pub mod ident;
pub mod matchable;
pub mod members;
pub mod parameter_matchable;
pub mod parsable;
pub mod properties;
pub mod property_generate_flags;
pub mod signals;
pub mod string_type;
pub mod virtual_methods;
pub mod work_mode;

pub use self::{
    child_properties::{ChildProperties, ChildProperty},
    config::Config,
    gobjects::GObject,
    property_generate_flags::PropertyGenerateFlags,
    string_type::StringType,
    work_mode::WorkMode,
};

use self::error::TomlHelper;
use log::warn;

fn get_cfg_condition(toml: &toml::Value, object_name: &str) -> Option<String> {
    let cfg_condition = toml.lookup("cfg_condition").and_then(toml::Value::as_str);
    let Some(sub_object_name) = object_name.split('.').nth(1) else {
        return cfg_condition.map(ToString::to_string);
    };
    if sub_object_name.starts_with("win32_") {
        match cfg_condition {
            Some("windows") => {
                warn!("\"object {object_name}\": No need to set `cfg_condition` to `windows` if name starts with `win32_`");
                Some("window".to_string())
            }
            None => Some("window".to_string()),
            Some(cfg) => Some(format!("{cfg},windows")),
        }
    } else if sub_object_name.starts_with("unix_") {
        match cfg_condition {
            Some("unix") => {
                warn!("\"object {object_name}\": No need to set `cfg_condition` to `unix` if name starts with `unix_`");
                Some("unix".to_string())
            }
            None => Some("unix".to_string()),
            Some(cfg) => Some(format!("{cfg},unix")),
        }
    } else {
        cfg_condition.map(ToString::to_string)
    }
}

fn get_object_cfg_condition(toml: &toml::Value, object_name: &str) -> Option<String> {
    let cfg_condition = toml.lookup("cfg_condition").and_then(toml::Value::as_str);
    let Some(sub_object_name) = object_name.split('.').nth(1) else {
        return cfg_condition.map(ToString::to_string);
    };
    if sub_object_name.starts_with("Win32") || sub_object_name.starts_with("GWin32") {
        match cfg_condition {
            Some("windows") => {
                warn!("\"object {object_name}\": No need to set `cfg_condition` to `windows` if object name starts with `Win32`");
                Some("windows".to_string())
            }
            None => Some("windows".to_string()),
            Some(cfg) => Some(format!("{cfg},windows")),
        }
    } else if sub_object_name.starts_with("Unix") || sub_object_name.starts_with("GUnix") {
        match cfg_condition {
            Some("unix") => {
                warn!("\"object {object_name}\": No need to set `cfg_condition` to `unix` if object name starts with `Unix`");
                Some("unix".to_string())
            }
            None => Some("unix".to_string()),
            Some(cfg) => Some(format!("{cfg},unix")),
        }
    } else {
        cfg_condition.map(ToString::to_string)
    }
}
