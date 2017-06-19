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
    FfiCallParameter{par: parameter_ffi_call_in::Parameter},
    FfiCallOutParameter{par: parameter_ffi_call_out::Parameter},
    //TODO: separate without return_value::Info
    FfiCallConversion{ret: return_value::Info, array_length: Option<(String, String)>, call: Box<Chunk>},
    Let{name: String, is_mut: bool, value: Box<Chunk>, type_: Option<Box<Chunk>>},
    Uninitialized,
    UninitializedNamed{name: String},
    NullPtr,
    NullMutPtr,
    Custom(String),
    Tuple(Vec<Chunk>, TupleMode),
    FromGlibConversion{mode: conversion_from_glib::Mode, array_length: Option<(String, String)>, value: Box<Chunk>},
    OptionalReturn{condition: String, value: Box<Chunk>},
    ErrorResultReturn{value: Box<Chunk>},
    AssertInitializedAndInMainThread,
    AssertSkipInitialized,
    Connect{signal: String, trampoline: String, in_trait: bool},
}

pub fn chunks(ch: Chunk) -> Vec<Chunk> {
    vec![ch]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TupleMode {
    Auto,      // "", "1", "(1,2)"
    WithUnit,  // "()", "1", "(1,2)"
    //Simple,    // "()", "(1)", "(1,2)"
}
