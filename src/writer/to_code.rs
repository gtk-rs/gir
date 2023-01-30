use std::fmt::Write;

use super::{primitives::*, safety_assertion_mode_to_str};
use crate::{
    chunk::{Chunk, Param, TupleMode},
    codegen::{translate_from_glib::TranslateFromGlib, translate_to_glib::TranslateToGlib},
    env::Env,
    nameutil::use_glib_type,
};

pub trait ToCode {
    fn to_code(&self, env: &Env) -> Vec<String>;
}

impl ToCode for Chunk {
    fn to_code(&self, env: &Env) -> Vec<String> {
        use crate::chunk::Chunk::*;
        match *self {
            Comment(ref chs) => comment_block(&chs.to_code(env)),
            Chunks(ref chs) => chs.to_code(env),
            BlockHalf(ref chs) => format_block("", "}", &chs.to_code(env)),
            UnsafeSmart(ref chs) => {
                format_block_smart("unsafe {", "}", &chs.to_code(env), " ", " ")
            }
            Unsafe(ref chs) => format_block("unsafe {", "}", &chs.to_code(env)),
            FfiCallTODO(ref name) => vec![format!("TODO: call {name}()")],
            FfiCall {
                ref name,
                ref params,
            } => {
                let prefix = format!("{name}(");
                // TODO: change to format_block or format_block_smart
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
                } else if par.is_uninitialized && !par.is_error {
                    format!("{}.as_mut_ptr()", par.name)
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
                    ret.translate_from_glib_as_function(env, array_length_name.as_deref());
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
                let type_string = if let Some(type_) = type_ {
                    let type_strings = type_.to_code(env);
                    format_block_one_line(": ", "", &type_strings, "", "")
                } else {
                    String::new()
                };
                let value_strings = value.to_code(env);
                let prefix = format!("let {modif}{name}{type_string} = ");
                let s = format_block_one_line(&prefix, ";", &value_strings, "", "");
                vec![s]
            }
            Uninitialized => vec!["mem::MaybeUninit::uninit()".into()],
            UninitializedNamed { ref name } => {
                let s = format!("{name}::uninitialized()");
                vec![s]
            }
            NullPtr => vec!["ptr::null()".into()],
            NullMutPtr => vec!["ptr::null_mut()".into()],
            Custom(ref string) => vec![string.clone()],
            Tuple(ref chs, ref mode) => {
                #[allow(deprecated)]
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
                    mode.translate_from_glib_as_function(env, array_length_name.as_deref());
                let s = format_block_one_line(&prefix, &suffix, &value_strings, "", "");
                vec![s]
            }
            OptionalReturn {
                ref condition,
                ref value,
            } => {
                let value_strings = value.to_code(env);
                let prefix = format!("if {condition} {{ Some(");
                let suffix = ") } else { None }";
                let s = format_block_one_line(&prefix, suffix, &value_strings, "", "");
                vec![s]
            }
            AssertErrorSanity => {
                let assert = format!(
                    "debug_assert_eq!(is_ok == {}, !error.is_null());",
                    use_glib_type(env, "ffi::GFALSE")
                );
                vec![assert]
            }
            ErrorResultReturn { ref ret, ref value } => {
                let mut lines = match ret {
                    Some(r) => r.to_code(env),
                    None => vec![],
                };
                let value_strings = value.to_code(env);
                let prefix = "if error.is_null() { Ok(";
                let suffix = ") } else { Err(from_glib_full(error)) }";
                let s = format_block_one_line(prefix, suffix, &value_strings, "", "");
                lines.push(s);
                lines
            }
            AssertInit(x) => vec![safety_assertion_mode_to_str(x).to_owned()],
            Connect {
                ref signal,
                ref trampoline,
                in_trait,
                is_detailed,
            } => {
                let mut v: Vec<String> = Vec::with_capacity(6);
                if is_detailed {
                    v.push(format!(
                        r#"let detailed_signal_name = detail.map(|name| {{ format!("{signal}::{{name}}\0") }});"#
                    ));
                    v.push(format!(
                        r#"let signal_name: &[u8] = detailed_signal_name.as_ref().map_or(&b"{signal}\0"[..], |n| n.as_bytes());"#
                    ));
                    v.push(
                        "connect_raw(self.as_ptr() as *mut _, signal_name.as_ptr() as *const _,"
                            .to_string(),
                    );
                } else {
                    v.push(format!(
                        "connect_raw(self.as_ptr() as *mut _, b\"{signal}\\0\".as_ptr() as *const _,"
                    ));
                }
                let self_str = if in_trait { "Self, " } else { "" };
                v.push(format!(
                    "\tSome(transmute::<_, unsafe extern \"C\" fn()>({trampoline}::<{self_str}F> as *const ())), Box_::into_raw(f))"
                ));
                v
            }
            Name(ref name) => vec![name.clone()],
            ExternCFunc {
                ref name,
                ref parameters,
                ref body,
                ref return_value,
                ref bounds,
            } => {
                let prefix = format!(r#"unsafe extern "C" fn {name}{bounds}("#);
                let suffix = ")".to_string();
                let params: Vec<_> = parameters
                    .iter()
                    .flat_map(|param| param.to_code(env))
                    .collect();
                let mut s = format_block_one_line(&prefix, &suffix, &params, "", ", ");
                if let Some(return_value) = return_value {
                    write!(s, " -> {return_value}").unwrap();
                }
                s.push_str(" {");
                let mut code = format_block("", "}", &body.to_code(env));
                code.insert(0, s);
                code
            }
            Cast {
                ref name,
                ref type_,
            } => vec![format!("{name} as {type_}")],
            Call {
                ref func_name,
                ref arguments,
            } => {
                let args: Vec<_> = arguments.iter().flat_map(|arg| arg.to_code(env)).collect();
                let s = format_block_one_line("(", ")", &args, "", ",");
                vec![format!("{func_name}{s};")]
            }
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
