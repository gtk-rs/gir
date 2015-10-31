use std::vec::Vec;

use analysis::return_value;
use super::conversion_from_glib;
use super::parameter_ffi_call_in;
use super::parameter_ffi_call_out;

pub enum Chunk {
    Comment(Vec<Chunk>),
    BlockHalf(Vec<Chunk>), //Block without open bracket, temporary
    UnsafeSmart(Vec<Chunk>),  //TODO: remove (will change generated results)
    Unsafe(Vec<Chunk>),
    FfiCallTODO(String),
    FfiCall{name: String, params: Vec<Chunk>},
    FfiCallParameter{par: parameter_ffi_call_in::Parameter, upcast: bool},
    FfiCallOutParameter{par: parameter_ffi_call_out::Parameter},
    //TODO: separate without return_value::Info
    FfiCallConversion{ret: return_value::Info, call: Box<Chunk>},
    Let{name: String, is_mut: bool, value: Box<Chunk>},
    Uninitialized,
    VariableValue{name: String},
    Tuple(Vec<Chunk>),
    FromGlibConversion{mode: conversion_from_glib::Mode, value: Box<Chunk>},
    OptionalReturn{condition: String, value: Box<Chunk>},
}

pub fn chunks(ch: Chunk) -> Vec<Chunk> {
    vec![ch]
}
