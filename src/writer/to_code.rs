use std::vec::Vec;
use chunk::Chunk;
use super::primitives::*;

pub trait ToCode {
    fn to_code(&self) -> Vec<String>;
}

impl ToCode for Chunk {
    fn to_code(&self) -> Vec<String> {
        use chunk::Chunk::*;
        match *self {
            Comment(ref chs) => comment_block(&chs.to_code()),
            BlockHalf(ref chs) => format_block("", "}", &chs.to_code()),
            Unsafe(ref chs) => format_block_smart("unsafe {", "}", &chs.to_code(), " ", " "),
            FfiCallTODO(ref name) => vec![format!("TODO: call ffi:{}()", name)],
        }
    }
}

impl ToCode for [Chunk] {
    fn to_code(&self) -> Vec<String> {
        let mut v = Vec::new();
        for ch in self {
            let strs = ch.to_code();
            //TODO: append
            for s in strs {
                v.push(s.clone());
            }
        }
        v
    }
}
