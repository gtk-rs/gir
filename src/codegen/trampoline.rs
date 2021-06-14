use super::{
    return_value::ToReturnValue, trampoline_from_glib::TrampolineFromGlib,
    trampoline_to_glib::TrampolineToGlib,
};
use crate::{
    analysis::{
        bounds::Bounds, ffi_type::ffi_type, ref_mode::RefMode, rust_type::RustType,
        trampoline_parameters::*, trampolines::Trampoline, try_from_glib::TryFromGlib,
    },
    consts::TYPE_PARAMETERS_START,
    env::Env,
    library::{self},
    nameutil::{use_glib_if_needed, use_gtk_type},
    traits::IntoString,
    writer::primitives::tabs,
};
use log::error;
use std::io::{Result, Write};

pub fn generate(
    w: &mut dyn Write,
    env: &Env,
    analysis: &Trampoline,
    in_trait: bool,
    indent: usize,
) -> Result<()> {
    let self_bound = in_trait
        .then(|| format!("{}: IsA<{}>, ", TYPE_PARAMETERS_START, analysis.type_name))
        .unwrap_or_default();

    let prepend = tabs(indent);
    let params_str = trampoline_parameters(env, analysis);
    let func_str = func_string(env, analysis, None, true);
    let ret_str = trampoline_returns(env, analysis);

    writeln!(
        w,
        "{}unsafe extern \"C\" fn {}<{}F: {}>({}, f: {}){} {{",
        prepend,
        analysis.name,
        self_bound,
        func_str,
        params_str,
        use_glib_if_needed(env, "ffi::gpointer"),
        ret_str,
    )?;
    writeln!(w, "{}\tlet f: &F = &*(f as *const F);", prepend)?;
    transformation_vars(w, env, analysis, &prepend)?;
    let call = trampoline_call_func(env, analysis, in_trait);
    writeln!(w, "{}\t{}", prepend, call)?;
    writeln!(w, "{}}}", prepend)?;

    Ok(())
}

pub fn func_string(
    env: &Env,
    analysis: &Trampoline,
    replace_self_bound: Option<&str>,
    closure: bool,
) -> String {
    let param_str = func_parameters(env, analysis, replace_self_bound, closure);
    let return_str = func_returns(env, analysis);

    if closure {
        let concurrency_str = match analysis.concurrency {
            // If an object can be Send to other threads, this means that
            // our callback will be called from whatever thread the object
            // is sent to. But it will only be ever owned by a single thread
            // at a time, so signals can only be emitted from one thread at
            // a time and Sync is not needed
            library::Concurrency::Send | library::Concurrency::SendUnique => " + Send",
            // If an object is Sync, it can be shared between threads, and as
            // such our callback can be called from arbitrary threads and needs
            // to be Send *AND* Sync
            library::Concurrency::SendSync => " + Send + Sync",
            library::Concurrency::None => "",
        };

        format!(
            "Fn({}){}{} + 'static",
            param_str.replace("glib::GString", "&str"),
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
    replace_self_bound: Option<&str>,
    closure: bool,
) -> String {
    let mut param_str = String::with_capacity(100);

    for (pos, par) in analysis.parameters.rust_parameters.iter().enumerate() {
        if pos == 0 {
            if let Some(replace_self_bound) = &replace_self_bound {
                param_str.push_str(par.ref_mode.for_rust_type());
                param_str.push_str(replace_self_bound.as_ref());
                continue;
            }
        } else {
            param_str.push_str(", ");
            if !closure {
                param_str.push_str(&format!("{}: ", par.name));
            }
        }

        let s = func_parameter(env, par, &analysis.bounds);
        param_str.push_str(&s);
    }

    param_str
}

fn func_parameter(env: &Env, par: &RustParameter, bounds: &Bounds) -> String {
    //TODO: restore mutable support
    let ref_mode = if par.ref_mode == RefMode::ByRefMut {
        RefMode::ByRef
    } else {
        par.ref_mode
    };

    match bounds.get_parameter_bound(&par.name) {
        Some(bound) => bound.full_type_parameter_reference(ref_mode, par.nullable),
        None => RustType::builder(env, par.typ)
            .direction(par.direction)
            .nullable(par.nullable)
            .ref_mode(ref_mode)
            .try_build_param()
            .into_string(),
    }
}

fn func_returns(env: &Env, analysis: &Trampoline) -> String {
    if analysis.ret.typ == Default::default() {
        String::new()
    } else if analysis.inhibit {
        " -> glib::signal::Inhibit".into()
    } else if let Some(return_type) =
        analysis
            .ret
            .to_return_value(env, &TryFromGlib::default(), true)
    {
        format!(" -> {}", return_type)
    } else {
        String::new()
    }
}

fn trampoline_parameters(env: &Env, analysis: &Trampoline) -> String {
    if analysis.is_notify {
        return format!(
            "{}, _param_spec: {}",
            trampoline_parameter(env, &analysis.parameters.c_parameters[0]),
            use_glib_if_needed(env, "ffi::gpointer"),
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

fn transformation_vars(
    w: &mut dyn Write,
    env: &Env,
    analysis: &Trampoline,
    prepend: &str,
) -> Result<()> {
    use crate::analysis::trampoline_parameters::TransformationType::*;
    for transform in &analysis.parameters.transformations {
        match transform.transformation {
            None => (),
            Borrow => (),
            TreePath => {
                let c_par = &analysis.parameters.c_parameters[transform.ind_c];
                writeln!(
                    w,
                    "{}\tlet {} = from_glib_full({}({}));",
                    prepend,
                    transform.name,
                    use_gtk_type(env, "ffi::gtk_tree_path_new_from_string"),
                    c_par.name
                )?;
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
        let par_str = transformation.trampoline_from_glib(env, need_downcast, *par.nullable);
        parameter_strs.push(par_str);
        need_downcast = false; //Only downcast first parameter
    }

    parameter_strs.join(", ")
}
