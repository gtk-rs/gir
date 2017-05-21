use std::io::{Result, Write};

use analysis;
use analysis::bounds::Bounds;
use analysis::functions::Visibility;
use chunk::{ffi_function_todo, Chunk};
use env::Env;
use super::function_body_chunk;
use super::general::{cfg_condition, doc_hidden, not_version_condition, version_condition};
use super::parameter::ToParameter;
use super::return_value::{out_parameters_as_return, ToReturnValue};
use writer::primitives::tabs;
use writer::ToCode;

pub fn generate(w: &mut Write, env: &Env, analysis: &analysis::functions::Info,
                in_trait: bool, only_declaration: bool, indent: usize) -> Result<()> {
    let mut commented = false;
    let mut comment_prefix = "";
    let mut pub_prefix = if in_trait { "" } else { "pub " };
    match analysis.visibility {
        Visibility::Public => {}
        Visibility::Comment => {
            commented = true;
            comment_prefix = "//";
        }
        Visibility::Private => {
            if in_trait {
                warn!("Generating trait method for private function {}", analysis.glib_name);
            }
            else {
                pub_prefix = "";
            }
        }
        Visibility::Hidden => return Ok(()),
    }
    let declaration = declaration(env, analysis);
    let suffix = if only_declaration { ";" } else { " {" };

    try!(writeln!(w, ""));
    try!(cfg_condition(w, &analysis.cfg_condition, commented, indent));
    try!(version_condition(w, env, analysis.version, commented, indent));
    try!(not_version_condition(w, analysis.not_version, commented, indent));
    try!(doc_hidden(w, analysis.doc_hidden, comment_prefix, indent));
    try!(writeln!(w, "{}{}{}{}{}", tabs(indent),
        comment_prefix, pub_prefix, declaration, suffix));

    if !only_declaration {
        let body = body_chunk(env, analysis).to_code(env);
        for s in body {
            try!(writeln!(w, "{}{}", tabs(indent), s));
        }
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

    let bounds = bounds(&analysis.bounds);

    let mut skipped = 0;
    for (pos, par) in analysis.parameters.iter().enumerate() {
        if outs_as_return && analysis.outs.iter().any(|p| p.name==par.name) {
            skipped += 1;
            continue;
        }
        if pos > skipped { param_str.push_str(", ") }
        let s = par.to_parameter(env, &analysis.bounds);
        param_str.push_str(&s);
    }

    format!("fn {}{}({}){}", analysis.name, bounds, param_str, return_str)
}

fn bounds(bounds: &Bounds) -> String {
    use analysis::bounds::BoundType::*;
    if bounds.is_empty() { return String::new() }
    let strs: Vec<String> = bounds.iter_lifetimes()
        .map(|s| format!("'{}", s))
        .chain(bounds.iter()
                     .map(|bound| match bound.bound_type {
                         IsA(Some(lifetime)) => format!("{}: IsA<{}> + '{}",
                                                        bound.alias, bound.type_str, lifetime),
                         IsA(None) => format!("{}: IsA<{}>", bound.alias, bound.type_str),
                         // This case should normally never happened
                         AsRef(Some(lifetime)) => format!("{}: AsRef<{}> + '{}",
                                                          bound.alias, bound.type_str, lifetime),
                         AsRef(None) => format!("{}: AsRef<{}>", bound.alias, bound.type_str),
                         Into(Some(l), _) => format!("{}: Into<Option<&'{} {}>>",
                                                     bound.alias, l, bound.type_str),
                         Into(None, _) => format!("{}: Into<Option<{}>>",
                                                  bound.alias, bound.type_str),
                     }))
        .collect();
    format!("<{}>", strs.join(", "))
}

pub fn body_chunk(env: &Env, analysis: &analysis::functions::Info) -> Chunk {
    if analysis.visibility == Visibility::Comment {
        return ffi_function_todo(&analysis.glib_name);
    }

    let outs_as_return = !analysis.outs.is_empty();
    let mut builder = function_body_chunk::Builder::new();
    builder.glib_name(&analysis.glib_name)
        .assertion(analysis.assertion)
        .ret(&analysis.ret)
        .outs_mode(analysis.outs.mode);

    for par in &analysis.parameters {
        if outs_as_return && analysis.outs.iter().any(|p| p.name==par.name) {
            builder.out_parameter(env, par);
        } else {
            builder.parameter(par);
        }
    }

    builder.generate()
}
