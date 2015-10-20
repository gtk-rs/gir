mod defines;
pub mod primitives;

pub use self::defines::{TAB, TAB_SIZE, MAX_TEXT_WIDTH};
use self::primitives::tabs;

//TODO: move to chunks
pub fn ffi_function_todo(name: &str) -> Vec<String> {
    let mut v = Vec::new();
    v.push(format!("//{}unsafe {{ TODO: call ffi:{}() }}",
                        tabs(1), name));
    v.push(format!("//}}"));
    v
}
