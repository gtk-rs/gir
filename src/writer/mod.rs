mod defines;
pub mod primitives;

pub use self::defines::{TAB, TAB_SIZE, MAX_TEXT_WIDTH};
use self::primitives::{comment_block, format_block, format_block_smart};

//TODO: move to chunks
pub fn ffi_function_todo(name: &str) -> Vec<String> {
    let call = vec![format!("TODO: call ffi:{}()", name)];
    let unsafe_ = format_block_smart("unsafe {", "}", &call, " ", " ");
    let block = format_block("", "}", &unsafe_);
    comment_block(&block)
}
