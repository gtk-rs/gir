use std::vec::Vec;

use chunk::{Chunk, TupleMode};
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
            Let{ref name, is_mut, ref value, ref type_} => {
                let modif = if is_mut { "mut " } else { "" };
                let type_string = if let Some(ref type_) = *type_ {
                    let type_strings = type_.to_code(env);
                    format_block_one_line(": ", "", &type_strings, "", "")
                } else {
                    "".to_owned()
                };
                let value_strings = value.to_code(env);
                let prefix = format!("let {}{}{} = ", modif, name, type_string);
                let s = format_block_one_line(&prefix, ";", &value_strings, "", "");
                vec![s]
            }
            Uninitialized => vec!["mem::uninitialized()".into()],
            UninitializedNamed{ ref name } => {
                let s = format!("{}::uninitialized()", name);
                vec![s]
            }
            NullMutPtr => vec!["ptr::null_mut()".into()],
            Custom(ref string) => vec![string.clone()],
            Tuple(ref chs, mode) => {
                let with_bracket = match mode {
                    TupleMode::Auto => chs.len() > 1,
                    TupleMode::WithUnit => chs.len() != 1,
//                    TupleMode::Simple => true,
                };
                let (prefix, suffix) = if with_bracket { ( "(", ")" ) } else { ( "", "" ) };
                let s = format_block_one_line(prefix, suffix, &chs.to_code(env), "", ", ");
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
            ErrorResultReturn{ref value} => {
                let value_strings = value.to_code(env);
                let prefix = "if error.is_null() { Ok(";
                let suffix = ") } else { Err(from_glib_full(error)) }";
                let s = format_block_one_line(&prefix, suffix, &value_strings, "", "");
                vec![s]
            }
            AssertInitializedAndInMainThread =>
                vec!["assert_initialized_main_thread!();".to_string()],
            AssertSkipInitialized =>
                vec!["skip_assert_initialized!();".to_string()],
            Connect{ref signal, ref trampoline, in_trait} => {
                let s1 = format!("connect(self.to_glib_none().0, \"{}\",", signal);
                let self_str = if in_trait { "::<Self>" } else { "" };
                let s2 = format!("\ttransmute({}{} as usize), Box::into_raw(f) as *mut _)", trampoline, self_str);
                vec![s1, s2]
            }
        }
    }
}

impl ToCode for [Chunk] {
    fn to_code(&self, env: &Env) -> Vec<String> {
        let mut v = Vec::new();
        for ch in self {
            let strs = ch.to_code(env);
            v.extend_from_slice(&strs);
        }
        v
    }
}
