use std::io::{Result, Write};

use env::Env;
use library;
use analysis::bounds::{BoundType, Bounds};
use analysis::ffi_type::ffi_type;
use analysis::ref_mode::RefMode;
use analysis::rust_type::parameter_rust_type;
use analysis::trampoline_parameters::*;
use analysis::trampolines::Trampoline;
use super::general::version_condition;
use super::return_value::ToReturnValue;
use super::trampoline_from_glib::TrampolineFromGlib;
use super::trampoline_to_glib::TrampolineToGlib;
use traits::IntoString;
use consts::TYPE_PARAMETERS_START;

pub fn generate(
    w: &mut Write,
    env: &Env,
    analysis: &Trampoline,
    in_trait: bool,
    object_name: &str,
) -> Result<()> {
    try!(writeln!(w));
    let (bounds, end) = if in_trait {
        (format!("<{}>", TYPE_PARAMETERS_START), "")
    } else {
        (String::new(), " {")
    };

    let params_str = trampoline_parameters(env, analysis);
    let func_str = func_string(env, analysis, None, true);
    let ret_str = trampoline_returns(env, analysis);

    try!(version_condition(w, env, analysis.version, false, 0));
    try!(writeln!(
        w,
        "unsafe extern \"C\" fn {}{}({}, f: glib_ffi::gpointer){}{}",
        analysis.name,
        bounds,
        params_str,
        ret_str,
        end
    ));
    if in_trait {
        try!(writeln!(
            w,
            "where {}: IsA<{}> {{",
            TYPE_PARAMETERS_START,
            object_name
        ));
    }
    try!(writeln!(w, "\tcallback_guard!();"));
    try!(writeln!(w, "\tlet f: &&({}) = transmute(f);", func_str));
    try!(transformation_vars(w, analysis));
    let call = trampoline_call_func(env, analysis, in_trait);
    try!(writeln!(w, "\t{}", call));
    try!(writeln!(w, "}}"));

    Ok(())
}

pub fn func_string(
    env: &Env,
    analysis: &Trampoline,
    bound_replace: Option<(char, &str)>,
    closure: bool,
) -> String {
    let param_str = func_parameters(env, analysis, bound_replace, closure);
    let return_str = func_returns(env, analysis);

    if closure {
        let concurrency_str = match analysis.concurrency {
            // If an object can be Send to other threads, this means that
            // our callback will be called from whatever thread the object
            // is sent to. But it will only be ever owned by a single thread
            // at a time, so signals can only be emitted from one thread at
            // a time and Sync is not needed
            library::Concurrency::Send => " + Send",
            // If an object is Sync, it can be shared between threads, and as
            // such our callback can be called from arbitrary threads and needs
            // to be Send *AND* Sync
            library::Concurrency::SendSync => " + Send + Sync",
            library::Concurrency::None => "",
        };

        format!(
            "Fn({}){}{} + 'static",
            param_str,
            return_str,
            concurrency_str
        )
    } else {
        format!("({}){}", param_str, return_str,)
    }
}

fn func_parameters(
    env: &Env,
    analysis: &Trampoline,
    bound_replace: Option<(char, &str)>,
    closure: bool,
) -> String {
    let mut param_str = String::with_capacity(100);

    for (pos, par) in analysis.parameters.rust_parameters.iter().enumerate() {
        if pos > 0 {
            param_str.push_str(", ");
            if !closure {
                param_str.push_str(&format!("{}: ", par.name));
            }
        } else if !closure {
            param_str.push_str("&self");
            continue;
        }

        let s = func_parameter(env, par, &analysis.bounds, bound_replace);
        param_str.push_str(&s);
    }

    param_str
}

fn func_parameter(
    env: &Env,
    par: &RustParameter,
    bounds: &Bounds,
    bound_replace: Option<(char, &str)>,
) -> String {
    //TODO: restore mutable support
    //let mut_str = if par.ref_mode == RefMode::ByRefMut { "mut " } else { "" };
    let mut_str = "";
    let ref_mode = if par.ref_mode == RefMode::ByRefMut {
        RefMode::ByRef
    } else {
        par.ref_mode
    };

    match bounds.get_parameter_alias_info(&par.name) {
        Some((t, bound_type)) => match bound_type {
            BoundType::NoWrapper => unreachable!(),
            BoundType::IsA(_) => if *par.nullable {
                format!("&Option<{}{}>", mut_str, t)
            } else if let Some((from, to)) = bound_replace {
                if from == t {
                    format!("&{}{}", mut_str, to)
                } else {
                    format!("&{}{}", mut_str, t)
                }
            } else {
                format!("&{}{}", mut_str, t)
            },
            BoundType::AsRef(_) | BoundType::Into(_, _) => t.to_string(),
        },
        None => {
            let rust_type =
                parameter_rust_type(env, par.typ, par.direction, par.nullable, ref_mode);
            rust_type.into_string().replace("Option<&", "&Option<")
        }
    }
}

fn func_returns(env: &Env, analysis: &Trampoline) -> String {
    if analysis.ret.typ == Default::default() {
        String::new()
    } else if analysis.inhibit {
        " -> Inhibit".into()
    } else {
        analysis.ret.to_return_value(env)
    }
}

fn trampoline_parameters(env: &Env, analysis: &Trampoline) -> String {
    if analysis.is_notify {
        return format!(
            "{}, _param_spec: glib_ffi::gpointer",
            trampoline_parameter(env, &analysis.parameters.c_parameters[0])
        );
    }

    let mut parameter_strs: Vec<String> = Vec::new();
    for par in &analysis.parameters.c_parameters {
        let par_str = trampoline_parameter(env, par);
        parameter_strs.push(par_str);
    }

    parameter_strs.join(", ")
}

fn trampoline_parameter(env: &Env, par: &CParameter) -> String {
    let ffi_type = ffi_type(env, par.typ, &par.c_type);
    format!("{}: {}", par.name, ffi_type.into_string())
}

fn trampoline_returns(env: &Env, analysis: &Trampoline) -> String {
    if analysis.ret.typ == Default::default() {
        String::new()
    } else {
        let ffi_type = ffi_type(env, analysis.ret.typ, &analysis.ret.c_type);
        format!(" -> {}", ffi_type.into_string())
    }
}

fn transformation_vars(w: &mut Write, analysis: &Trampoline) -> Result<()> {
    use analysis::trampoline_parameters::TransformationType::*;
    for transform in &analysis.parameters.transformations {
        match transform.transformation {
            None => (),
            Borrow => (),
            TreePath => {
                let c_par = &analysis.parameters.c_parameters[transform.ind_c];
                try!(writeln!(
                    w,
                    "\tlet {} = from_glib_full(ffi::gtk_tree_path_new_from_string({}));",
                    transform.name,
                    c_par.name
                ));
            }
        }
    }
    Ok(())
}

fn trampoline_call_func(env: &Env, analysis: &Trampoline, in_trait: bool) -> String {
    let params = trampoline_call_parameters(env, analysis, in_trait);
    let ret = if analysis.ret.typ == Default::default() {
        String::new()
    } else {
        analysis.ret.trampoline_to_glib(env)
    };
    format!("f({}){}", params, ret)
}

fn trampoline_call_parameters(env: &Env, analysis: &Trampoline, in_trait: bool) -> String {
    let mut need_downcast = in_trait;
    let mut parameter_strs: Vec<String> = Vec::new();
    for (ind, par) in analysis.parameters.rust_parameters.iter().enumerate() {
        let transformation = match analysis.parameters.get(ind) {
            Some(transformation) => transformation,
            None => {
                error!("No transformation for {}", par.name);
                continue;
            }
        };
        let par_str = transformation.trampoline_from_glib(env, need_downcast);
        parameter_strs.push(par_str);
        need_downcast = false; //Only downcast first parameter
    }

    parameter_strs.join(", ")
}
