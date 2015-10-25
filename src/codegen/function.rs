use std::io::{Result, Write};

use analysis;
use analysis::upcasts::Upcasts;
use chunk::{ffi_function_todo, Chunk};
use env::Env;
use super::function_body;
use super::function_body_chunk;
use super::general::version_condition;
use super::parameter::ToParameter;
use super::return_value::{out_parameters_as_return, ToReturnValue};
use super::translate_from_glib::TranslateFromGlib;
use super::translate_to_glib::TranslateToGlib;
use writer::primitives::{format_block, tabs};
use writer::ToCode;

pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::functions::Info,
    in_trait: bool, only_declaration: bool, indent: usize) -> Result<()> {

    let comment_prefix = if analysis.comented { "//" } else { "" };
    let pub_prefix = if in_trait { "" } else { "pub " };
    let declaration = declaration(env, analysis);
    let suffix = if only_declaration { ";" } else { " {" };

    try!(version_condition(w, &env.config.library_name,
        env.config.min_cfg_version, analysis.version, analysis.comented, indent));
    try!(writeln!(w, "{}{}{}{}{}", tabs(indent),
        comment_prefix, pub_prefix, declaration, suffix));

    if !only_declaration {
        let body = if analysis.comented {
            let ch = ffi_function_todo(&analysis.glib_name);
            ch.to_code()
        } else if let Some(chunk) = body_chunk(env, analysis, in_trait) {
            chunk.to_code()
        } else {
            let body = body(env, analysis, in_trait);
            format_block("", "}", &body)
        };
        for s in body {
            try!(writeln!(w, "{}{}", tabs(indent), s));
        }
        try!(writeln!(w, ""));
    }

    Ok(())
}

pub fn declaration(env: &Env, analysis: &analysis::functions::Info) -> String {
    let outs_as_return = !analysis.outs.is_empty();
    let return_str = if outs_as_return {
        out_parameters_as_return(env, analysis)
    } else {
        analysis.ret.to_return_value(env)
    };
    let mut param_str = String::with_capacity(100);

    let upcasts = upcasts(&analysis.upcasts);

    for (pos, par) in analysis.parameters.iter().enumerate() {
        if outs_as_return && analysis.outs.iter().any(|p| p.name==par.name) {
            continue;
        }
        if pos > 0 { param_str.push_str(", ") }
        let s = par.to_parameter(env, &analysis.upcasts);
        param_str.push_str(&s);
    }

    format!("fn {}{}({}){}", analysis.name, upcasts, param_str, return_str)
}

fn upcasts(upcasts: &Upcasts) -> String {
    if upcasts.is_empty() { return String::new() }
    let strs: Vec<String> = upcasts.iter()
        .map(|upcast| { format!("{}: Upcast<{}>", upcast.1, upcast.2)})
        .collect();
    format!("<{}>", strs.join(", "))
}

pub fn body_chunk(env: &Env, analysis: &analysis::functions::Info,
    in_trait: bool) -> Option<Chunk> {
    let outs_as_return = !analysis.outs.is_empty();
    let mut builder = function_body_chunk::Builder::new();
    builder.glib_name(&analysis.glib_name)
        .from_glib(analysis.ret.translate_from_glib_as_function(env))
        .outs_mode(analysis.outs.mode);

    //TODO: change to map on parameters with pass Vec<String> to builder
    for par in &analysis.parameters {
        if outs_as_return && analysis.outs.iter().any(|p| p.name==par.name) {
            let name = par.name.clone();
            let (prefix, suffix) = par.translate_from_glib_as_function(env);
            builder.out_parameter(name, prefix, suffix);
        } else {
            let upcast = in_trait && par.instance_parameter
                || analysis.upcasts.iter().any(|&(ref name, _, _)| name == &par.name);
            let s = par.translate_to_glib(&env.library, upcast);
            builder.parameter(s);
        }
    }

    builder.generate()
}

pub fn body(env: &Env, analysis: &analysis::functions::Info,
    in_trait: bool) -> Vec<String> {
    let outs_as_return = !analysis.outs.is_empty();
    let mut builder = function_body::Builder::new();
    builder.glib_name(&analysis.glib_name)
        .from_glib(analysis.ret.translate_from_glib_as_function(env))
        .outs_mode(analysis.outs.mode);

    //TODO: change to map on parameters with pass Vec<String> to builder
    for par in &analysis.parameters {
        if outs_as_return && analysis.outs.iter().any(|p| p.name==par.name) {
            let name = par.name.clone();
            let (prefix, suffix) = par.translate_from_glib_as_function(env);
            builder.out_parameter(name, prefix, suffix);
        } else {
            let upcast = in_trait && par.instance_parameter
                || analysis.upcasts.iter().any(|&(ref name, _, _)| name == &par.name);
            let s = par.translate_to_glib(&env.library, upcast);
            builder.parameter(s);
        }
    }

    builder.generate()
}
