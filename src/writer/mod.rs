mod defines;
pub mod primitives;
pub mod to_code; // TODO:remove pub
pub mod untabber;

pub use self::{
    defines::{MAX_TEXT_WIDTH, TAB, TAB_SIZE},
    to_code::ToCode,
};
use crate::analysis::safety_assertion_mode::SafetyAssertionMode;

pub fn safety_assertion_mode_to_str(s: SafetyAssertionMode) -> &'static str {
    match s {
        SafetyAssertionMode::None => "",
        SafetyAssertionMode::NotInitialized => "assert_not_initialized!();",
        SafetyAssertionMode::Skip => "skip_assert_initialized!();",
        SafetyAssertionMode::InMainThread => "assert_initialized_main_thread!();",
    }
}
