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
pub mod signals;
pub mod work_mode;
pub mod constants;

pub use self::config::Config;
pub use self::external_libraries::ExternalLibrary;
pub use self::work_mode::WorkMode;
pub use self::child_properties::{ChildProperties, ChildProperty};
