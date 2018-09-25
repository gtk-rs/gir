mod child_properties;
#[cfg_attr(feature = "cargo-clippy", allow(module_inception))]
pub mod config;
pub mod error;
mod external_libraries;
pub mod parameter_matchable;
pub mod functions;
pub mod gobjects;
pub mod ident;
pub mod matchable;
pub mod members;
pub mod parsable;
pub mod properties;
pub mod property_generate_flags;
pub mod signals;
pub mod string_type;
pub mod work_mode;
pub mod constants;
pub mod derives;

pub use self::config::Config;
pub use self::external_libraries::ExternalLibrary;
pub use self::gobjects::GObject;
pub use self::property_generate_flags::PropertyGenerateFlags;
pub use self::work_mode::WorkMode;
pub use self::child_properties::{ChildProperties, ChildProperty};
pub use self::string_type::StringType;
