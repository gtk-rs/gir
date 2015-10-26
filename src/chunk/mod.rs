mod chunk;
pub mod parameter_ffi_call_in;

pub use self::chunk::{chunks, Chunk};

pub fn ffi_function_todo(name: &str) -> Chunk {
    let call = Chunk::FfiCallTODO(name.into());
    let unsafe_ = Chunk::UnsafeSmart(chunks(call));
    let block = Chunk::BlockHalf(chunks(unsafe_));
    Chunk::Comment(chunks(block))
}
