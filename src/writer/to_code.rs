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
            UnsafeSmart(ref chs) => format_block_smart("unsafe {", "}", &chs.to_code(), " ", " "),
            Unsafe(ref chs) => format_block("unsafe {", "}", &chs.to_code()),
            FfiCallTODO(ref name) => vec![format!("TODO: call ffi:{}()", name)],
            FfiCall{ref name, ref prefix, ref suffix, ref params} => {
                let prefix = format!("{}ffi::{}(", prefix, name);
                let suffix = format!("){}", suffix);
                //TODO: change to format_block or format_block_smart
                let s = format_block_one_line(&prefix, &suffix, &params.to_code(), "", ", ");
                vec![s]
            }
            FfiCallParameter(ref text) => vec![text.clone()],
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
