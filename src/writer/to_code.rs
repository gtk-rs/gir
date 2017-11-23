use std::vec::Vec;

use chunk::{Chunk, Param, TupleMode};
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
            UnsafeSmart(ref chs) => {
                format_block_smart("unsafe {", "}", &chs.to_code(env), " ", " ")
            }
            Unsafe(ref chs) => format_block("unsafe {", "}", &chs.to_code(env)),
            FfiCallTODO(ref name) => vec![format!("TODO: call ffi::{}()", name)],
            FfiCall {
                ref name,
                ref params,
            } => {
                let prefix = if name.find("::").is_some() {
                    format!("{}(", name)
                } else {
                    format!("ffi::{}(", name)
                };
                //TODO: change to format_block or format_block_smart
                let s = format_block_one_line(&prefix, ")", &params.to_code(env), "", ", ");
                vec![s]
            }
            FfiCallParameter {
                ref transformation_type,
            } => {
                let s = transformation_type.translate_to_glib();
                vec![s]
            }
            FfiCallOutParameter { ref par } => {
                let s = if par.caller_allocates {
                    format!("{}.to_glib_none_mut().0", par.name)
                } else {
                    format!("&mut {}", par.name)
                };
                vec![s]
            }
            FfiCallConversion {
                ref ret,
                ref array_length_name,
                ref call,
            } => {
                let call_strings = call.to_code(env);
                let (prefix, suffix) =
                    ret.translate_from_glib_as_function(env, array_length_name.as_ref());
                let s = format_block_one_line(&prefix, &suffix, &call_strings, "", "");
                vec![s]
            }
            Let {
                ref name,
                is_mut,
                ref value,
                ref type_,
            } => {
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
            UninitializedNamed { ref name } => {
                let s = format!("{}::uninitialized()", name);
                vec![s]
            }
            NullPtr => vec!["ptr::null()".into()],
            NullMutPtr => vec!["ptr::null_mut()".into()],
            Custom(ref string) => vec![string.clone()],
            Tuple(ref chs, mode) => {
                let with_bracket = match mode {
                    TupleMode::Auto => chs.len() > 1,
                    TupleMode::WithUnit => chs.len() != 1,
                    TupleMode::Simple => true,
                };
                let (prefix, suffix) = if with_bracket { ("(", ")") } else { ("", "") };
                let s = format_block_one_line(prefix, suffix, &chs.to_code(env), "", ", ");
                vec![s]
            }
            FromGlibConversion {
                ref mode,
                ref array_length_name,
                ref value,
            } => {
                let value_strings = value.to_code(env);
                let (prefix, suffix) =
                    mode.translate_from_glib_as_function(env, array_length_name.as_ref());
                let s = format_block_one_line(&prefix, &suffix, &value_strings, "", "");
                vec![s]
            }
            OptionalReturn {
                ref condition,
                ref value,
            } => {
                let value_strings = value.to_code(env);
                let prefix = format!("if {} {{ Some(", condition);
                let suffix = ") } else { None }";
                let s = format_block_one_line(&prefix, suffix, &value_strings, "", "");
                vec![s]
            }
            ErrorResultReturn { ref value } => {
                let value_strings = value.to_code(env);
                let prefix = "if error.is_null() { Ok(";
                let suffix = ") } else { Err(from_glib_full(error)) }";
                let s = format_block_one_line(prefix, suffix, &value_strings, "", "");
                vec![s]
            }
            AssertInitializedAndInMainThread => {
                vec!["assert_initialized_main_thread!();".to_string()]
            }
            AssertSkipInitialized => vec!["skip_assert_initialized!();".to_string()],
            Connect {
                ref signal,
                ref trampoline,
                in_trait,
            } => {
                let s1 = format!("connect(self.to_glib_none().0, \"{}\",", signal);
                let self_str = if in_trait { "::<Self>" } else { "" };
                let s2 = format!(
                    "\ttransmute({}{} as usize), Box_::into_raw(f) as *mut _)",
                    trampoline,
                    self_str
                );
                vec![s1, s2]
            }
            New { ref name } => vec![format!("{}::new()", name)],
            Name(ref name) => vec![name.clone()],
            BoxFn { ref typ } => vec![format!("let user_data: Box<Box<{}>> = Box::new(Box::new(callback));", typ)],
            ExternCFunc { ref name, ref parameters, ref body } => {
                let prefix = format!(r#"extern "C" fn {}("#, name);
                let suffix = ")".to_string();
                let params: Vec<_> = parameters.iter()
                    .flat_map(|param| param.to_code(env))
                    .collect();
                let s = format_block_one_line(&prefix, &suffix, &params, "", ", ");
                let mut code = format_block_smart("{", "}", &body.to_code(env), " ", " ");
                code.insert(0, s);
                code
            },
            OutParam(ref name) => vec![format!("&mut {}", name)],
            Cast { ref name, ref type_ } => vec![format!("{} as {}", name, type_)],
            Transmute(ref chunk) => {
                let s = format_block_one_line("", "", &chunk.to_code(env), "", "");
                vec![format!("transmute({})", s)]
            },
            BoxedRef(ref typ) => vec![format!("&Box<{}>", typ)],
            Call { ref func_name, ref arguments } => {
                let args: Vec<_> = arguments.iter()
                    .flat_map(|arg| arg.to_code(env))
                    .collect();
                let s = format_block_one_line("(", ")", &args, "", ",");
                vec![format!("{}{};", func_name, s)]
            },
        }
    }
}

impl ToCode for Param {
    fn to_code(&self, _env: &Env) -> Vec<String> {
        vec![format!("{}: {}", self.name, self.typ)]
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
