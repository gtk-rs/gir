use super::{conversion_from_glib, parameter_ffi_call_out};
use crate::analysis::{
    function_parameters::TransformationType, return_value,
    safety_assertion_mode::SafetyAssertionMode,
};

#[derive(Clone, Debug)]
pub enum Chunk {
    Comment(Vec<Chunk>),
    Chunks(Vec<Chunk>),
    BlockHalf(Vec<Chunk>),   // Block without open bracket, temporary
    UnsafeSmart(Vec<Chunk>), // TODO: remove (will change generated results)
    Unsafe(Vec<Chunk>),
    #[allow(clippy::upper_case_acronyms)]
    FfiCallTODO(String),
    FfiCall {
        name: String,
        params: Vec<Chunk>,
    },
    FfiCallParameter {
        transformation_type: TransformationType,
    },
    FfiCallOutParameter {
        par: parameter_ffi_call_out::Parameter,
    },
    // TODO: separate without return_value::Info
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
    UninitializedNamed {
        name: String,
    },
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
    AssertErrorSanity,
    ErrorResultReturn {
        ret: Option<Box<Chunk>>,
        value: Box<Chunk>,
    },
    AssertInit(SafetyAssertionMode),
    Connect {
        signal: String,
        trampoline: String,
        in_trait: bool,
        is_detailed: bool,
    },
    Name(String),
    ExternCFunc {
        name: String,
        parameters: Vec<Param>,
        body: Box<Chunk>,
        return_value: Option<String>,
        bounds: String,
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

impl Chunk {
    pub fn is_uninitialized(&self) -> bool {
        matches!(*self, Self::Uninitialized)
    }
}

#[derive(Clone, Debug)]
pub struct Param {
    pub name: String,
    pub typ: String,
}

pub fn chunks(ch: Chunk) -> Vec<Chunk> {
    vec![ch]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TupleMode {
    Auto,     // "", "1", "(1,2)"
    WithUnit, // "()", "1", "(1,2)"
    #[deprecated]
    #[allow(dead_code)]
    Simple, // "()", "(1)", "(1,2)"
}
