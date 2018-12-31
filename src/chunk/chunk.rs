use std::vec::Vec;

use analysis::function_parameters::TransformationType;
use analysis::return_value;
use super::conversion_from_glib;
use super::parameter_ffi_call_out;

pub enum Chunk {
    Comment(Vec<Chunk>),
    Chunks(Vec<Chunk>),
    BlockHalf(Vec<Chunk>),   //Block without open bracket, temporary
    UnsafeSmart(Vec<Chunk>), //TODO: remove (will change generated results)
    Unsafe(Vec<Chunk>),
    FfiCallTODO(String),
    FfiCall { name: String, params: Vec<Chunk> },
    FfiCallParameter {
        transformation_type: TransformationType,
    },
    FfiCallOutParameter {
        par: parameter_ffi_call_out::Parameter,
    },
    //TODO: separate without return_value::Info
    FfiCallConversion {
        ret: return_value::Info,
        array_length_name: Option<String>,
        call: Box<Chunk>,
    },
    Let {
        name: String,
        is_mut: bool,
        value: Box<Chunk>,
        type_: Option<Box<Chunk>>,
    },
    Uninitialized,
    UninitializedNamed { name: String },
    NullPtr,
    NullMutPtr,
    Custom(String),
    Tuple(Vec<Chunk>, TupleMode),
    FromGlibConversion {
        mode: conversion_from_glib::Mode,
        array_length_name: Option<String>,
        value: Box<Chunk>,
    },
    OptionalReturn {
        condition: String,
        value: Box<Chunk>,
    },
    ErrorResultReturn { value: Box<Chunk> },
    AssertInitializedAndInMainThread,
    AssertSkipInitialized,
    Connect {
        signal: String,
        trampoline: String,
        in_trait: bool,
    },
    Name(String),
    BoxFn {
        name: Option<String>,
        typ: String,
    },
    ExternCFunc {
        name: String,
        parameters: Vec<Param>,
        body: Box<Chunk>,
    },
    Cast {
        name: String,
        type_: String,
    },
    Call {
        func_name: String,
        arguments: Vec<Chunk>,
    },
}

pub struct Param {
    pub name: String,
    pub typ: String,
}

pub fn chunks(ch: Chunk) -> Vec<Chunk> {
    vec![ch]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TupleMode {
    Auto, // "", "1", "(1,2)"
    WithUnit, // "()", "1", "(1,2)"
    #[deprecated]
    #[allow(dead_code)]
    Simple,    // "()", "(1)", "(1,2)"
}
