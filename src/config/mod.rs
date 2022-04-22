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
pub mod work_mode;

pub use self::{
    child_properties::{ChildProperties, ChildProperty},
    config::Config,
    external_libraries::ExternalLibrary,
    gobjects::GObject,
    property_generate_flags::PropertyGenerateFlags,
    string_type::StringType,
    work_mode::WorkMode,
};
