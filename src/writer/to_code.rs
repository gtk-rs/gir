use std::vec::Vec;

use chunk::Chunk;
use codegen::translate_to_glib::TranslateToGlib;
use env::Env;
use super::primitives::*;

pub trait ToCode {
    fn to_code(&self, env: &Env) -> Vec<String>;
}

impl ToCode for Chunk {
    fn to_code(&self, env: &Env) -> Vec<String> {
        use chunk::Chunk::*;
        match *self {
            Comment(ref chs) => comment_block(&chs.to_code(env)),
            BlockHalf(ref chs) => format_block("", "}", &chs.to_code(env)),
            UnsafeSmart(ref chs) => format_block_smart("unsafe {", "}", &chs.to_code(env), " ", " "),
            Unsafe(ref chs) => format_block("unsafe {", "}", &chs.to_code(env)),
            FfiCallTODO(ref name) => vec![format!("TODO: call ffi:{}()", name)],
            FfiCall{ref name, ref prefix, ref suffix, ref params} => {
                let prefix = format!("{}ffi::{}(", prefix, name);
                let suffix = format!("){}", suffix);
                //TODO: change to format_block or format_block_smart
                let s = format_block_one_line(&prefix, &suffix, &params.to_code(env), "", ", ");
                vec![s]
            }
            FfiCallParameter{ref par, upcast } => {
                let s = par.translate_to_glib(&env.library, upcast);
                vec![s]
            },
        }
    }
}

impl ToCode for [Chunk] {
    fn to_code(&self, env: &Env) -> Vec<String> {
        let mut v = Vec::new();
        for ch in self {
            let strs = ch.to_code(env);
            //TODO: append
            for s in strs {
                v.push(s.clone());
            }
        }
        v
    }
}
