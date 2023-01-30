use crate::env::Env;

#[allow(clippy::module_inception)]
mod chunk;
pub mod conversion_from_glib;
pub mod parameter_ffi_call_out;

pub use self::chunk::{chunks, Chunk, Param, TupleMode};

pub fn ffi_function_todo(env: &Env, name: &str) -> Chunk {
    let sys_crate_name = env.main_sys_crate_name();
    let call = Chunk::FfiCallTODO(format!("{sys_crate_name}:{name}"));
    let unsafe_ = Chunk::UnsafeSmart(chunks(call));
    let block = Chunk::BlockHalf(chunks(unsafe_));
    Chunk::Comment(chunks(block))
}
