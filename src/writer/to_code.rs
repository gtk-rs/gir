use std::vec::Vec;

use chunk::Chunk;
use codegen::translate_from_glib::TranslateFromGlib;
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
            FfiCallTODO(ref name) => vec![format!("TODO: call ffi::{}()", name)],
            FfiCall{ref name, ref params} => {
                let prefix = format!("ffi::{}(", name);
                //TODO: change to format_block or format_block_smart
                let s = format_block_one_line(&prefix, ")", &params.to_code(env), "", ", ");
                vec![s]
            }
            FfiCallParameter{ref par} => {
                let s = par.translate_to_glib(&env.library);
                vec![s]
            }
            FfiCallOutParameter{ref par} => {
                let s = if par.caller_allocates {
                    format!("{}.to_glib_none_mut().0", par.name)
                } else {
                    format!("&mut {}", par.name)
                };
                vec![s]
            }
            FfiCallConversion{ref ret, ref call} => {
                let call_strings = call.to_code(env);
                let (prefix, suffix) = ret.translate_from_glib_as_function(env);
                let s = format_block_one_line(&prefix, &suffix, &call_strings, "", "");
                vec![s]
            }
            Let{ref name, is_mut, ref value} => {
                let modif = if is_mut { "mut " } else { "" };
                let value_strings = value.to_code(env);
                let prefix = format!("let {}{} = ", modif, name);
                let s = format_block_one_line(&prefix, ";", &value_strings, "", "");
                vec![s]
            }
            Uninitialized => vec!["mem::uninitialized()".into()],
            UninitializedNamed{ ref name } => {
                let s = format!("{}::uninitialized()", name);
                vec![s]
            }
            NullMutPtr => vec!["ptr::null_mut()".into()],
            VariableValue{ref name} => vec![name.clone()],
            Tuple(ref chs) => {
                let s = format_block_one_line("(", ")", &chs.to_code(env), "", ", ");
                vec![s]
            }
            FromGlibConversion{ref mode, ref value} => {
                let value_strings = value.to_code(env);
                let (prefix, suffix) = mode.translate_from_glib_as_function(env);
                let s = format_block_one_line(&prefix, &suffix, &value_strings, "", "");
                vec![s]
            }
            OptionalReturn{ref condition, ref value} => {
                let value_strings = value.to_code(env);
                let prefix = format!("if {} {{ Some(", condition);
                let suffix = ") } else { None }";
                let s = format_block_one_line(&prefix, suffix, &value_strings, "", "");
                vec![s]
            }
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
