use super::gi_docgen;
use crate::{config::gobjects::GStatus, library::FunctionKind, nameutil, Env};
use once_cell::sync::Lazy;
use regex::{Captures, Match, Regex};

const LANGUAGE_SEP_BEGIN: &str = "<!-- language=\"";
const LANGUAGE_SEP_END: &str = "\" -->";
const LANGUAGE_BLOCK_BEGIN: &str = "|[";
const LANGUAGE_BLOCK_END: &str = "\n]|";

pub fn reformat_doc(input: &str, env: &Env, in_type: &str) -> String {
    code_blocks_transformation(input, env, in_type)
}

fn try_split<'a>(src: &'a str, needle: &str) -> (&'a str, Option<&'a str>) {
    match src.find(needle) {
        Some(pos) => (&src[..pos], Some(&src[pos + needle.len()..])),
        None => (src, None),
    }
}

fn code_blocks_transformation(mut input: &str, env: &Env, in_type: &str) -> String {
    let mut out = String::with_capacity(input.len());

    loop {
        input = match try_split(input, LANGUAGE_BLOCK_BEGIN) {
            (before, Some(after)) => {
                out.push_str(&format(before, env, in_type));
                if let (before, Some(after)) =
                    try_split(get_language(after, &mut out), LANGUAGE_BLOCK_END)
                {
                    out.push_str(before);
                    out.push_str("\n```");
                    after
                } else {
                    after
                }
            }
            (before, None) => {
                out.push_str(&format(before, env, in_type));
                return out;
            }
        };
    }
}

fn get_language<'a>(entry: &'a str, out: &mut String) -> &'a str {
    if let (_, Some(after)) = try_split(entry, LANGUAGE_SEP_BEGIN) {
        if let (before, Some(after)) = try_split(after, LANGUAGE_SEP_END) {
            out.push_str(&format!("\n```{}", before));
            return after;
        }
    }
    out.push_str("\n```text");
    entry
}

fn format(input: &str, env: &Env, in_type: &str) -> String {
    let mut ret = String::with_capacity(input.len());
    // We run gi_docgen first because it's super picky about the types it replaces
    let no_c_types_re = gi_docgen::replace_c_types(input, env, in_type);
    ret.push_str(&replace_c_types(&no_c_types_re, env, in_type));
    ret
}

static SYMBOL: Lazy<Regex> = Lazy::new(|| Regex::new(r"([#%])(\w+\b)([:.]+[\w-]+\b)?").unwrap());
static FUNCTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([@#%])?(\w+\b[:.]+)?(\b[a-z0-9_]+)\(\)").unwrap());
// **note**
// The optional . at the end is to make the regex more relaxed for some weird broken cases on gtk3's docs
// it doesn't hurt other docs so please don't drop it
static GDK_GTK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"`([^\(:])?((G[dts]k|Pango)\w+\b)(\.)?`").unwrap());
static TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[\w/-]+>").unwrap());
static SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ ]{2,}").unwrap());

fn replace_c_types(entry: &str, env: &Env, in_type: &str) -> String {
    let symbols = env.symbols.borrow();
    let out = FUNCTION.replace_all(entry, |caps: &Captures<'_>| {
        let name = &caps[3];
        find_function(name, env)
    });

    let out = SYMBOL.replace_all(&out, |caps: &Captures<'_>| {
        let prefix = &caps[1];
        if prefix == "#" {
            let symbol_name = &caps[2];
            let func_name = caps.get(3).map(|m| m.as_str().to_string());
            format!("{}", symbol_name)
        } else {
            // would be equal to "%"
            let symbol_name = &caps[2];
            find_constant_or_variant(symbol_name, env)
        }
    });
    let out = GDK_GTK.replace_all(&out, |caps: &Captures<'_>| find_struct(&caps[2], env));
    let out = TAGS.replace_all(&out, "`$0`");
    SPACES.replace_all(&out, " ").into_owned()
}

fn find_constant_or_variant(symbol: &str, env: &Env) -> String {
    let symbols = env.symbols.borrow();
    if let Some(const_info) = env
        .analysis
        .constants
        .iter()
        .find(|c| c.glib_name == symbol)
    {
        // for whatever reason constants are not part of the symbols list
        format!("[{name}](crate::{name})", name = const_info.name)
    } else if let Some((flag_info, member_info)) = env.analysis.flags.iter().find_map(|f| {
        f.type_(&env.library)
            .members
            .iter()
            .find(|m| m.c_identifier == symbol)
            .map(|m| (f, m))
    }) {
        let sym = symbols.by_tid(flag_info.type_id).unwrap();
        format!(
            "[{flag_name}::{member_name}](crate::{parent}{member_name})",
            member_name = nameutil::bitfield_member_name(&member_info.name),
            flag_name = flag_info.name,
            parent = sym.parent()
        )
    } else if let Some((enum_info, member_info)) = env.analysis.enumerations.iter().find_map(|e| {
        e.type_(&env.library)
            .members
            .iter()
            .find(|m| m.c_identifier == symbol)
            .map(|m| (e, m))
    }) {
        let sym = symbols.by_tid(enum_info.type_id).unwrap();
        format!(
            "[{enum_name}::{member}](crate::{parent}{member})",
            enum_name = enum_info.name,
            member = nameutil::enum_member_name(&member_info.name),
            parent = sym.parent()
        )
    } else {
        format!("`{}`", symbol)
    }
}

fn find_struct(name: &str, env: &Env) -> String {
    let symbols = env.symbols.borrow();

    let symbol = if let Some(obj) = env
        .analysis
        .objects
        .iter()
        .find(|(_, o)| o.c_type == name)
        .map(|(_, o)| o)
    {
        symbols.by_tid(obj.type_id)
    } else if let Some(record) = env
        .analysis
        .records
        .iter()
        .find(|(full_name, _)| full_name == &name)
        .map(|(_, r)| r)
    {
        symbols.by_tid(record.type_id)
    } else {
        None
    };
    symbol
        .map(|sym| format!("[{name}](crate::{name})", name = sym.full_rust_name()))
        .unwrap_or_else(|| name.to_string())
}

/// Find a function in all the possible items, if not found return the original name surrounded with backsticks
/// A function can either be a struct/interface/record method, a global function or maybe a virtual function
fn find_function(name: &str, env: &Env) -> String {
    let symbols = env.symbols.borrow();
    let is_obj_func = env.analysis.objects.iter().find_map(|(_, obj_info)| {
        obj_info
            .functions
            .iter()
            .filter(|fn_info| fn_info.status != GStatus::Ignore)
            .filter(|fn_info| !fn_info.is_special() && !fn_info.is_async_finish(env))
            .find(|fn_info| fn_info.glib_name == name)
            .map(|fn_info| (obj_info, fn_info))
    });
    let is_record_func = env.analysis.records.iter().find_map(|(_, record_info)| {
        record_info
            .functions
            .iter()
            .filter(|fn_info| fn_info.status != GStatus::Ignore)
            .filter(|fn_info| !fn_info.is_special() && !fn_info.is_async_finish(env))
            .find(|fn_info| fn_info.glib_name == name)
            .map(|fn_info| (record_info, fn_info))
    });
    let is_globa_func = env.analysis.global_functions.as_ref().and_then(|info| {
        info.functions
            .iter()
            .filter(|fn_info| fn_info.status != GStatus::Ignore)
            .filter(|fn_info| !fn_info.is_special() && !fn_info.is_async_finish(env))
            .find(|fn_info| fn_info.glib_name == name)
    });

    if let Some((obj_info, fn_info)) = is_obj_func {
        let sym = symbols.by_tid(obj_info.type_id).unwrap(); // we are sure the object exists
        let (type_name, visible_type_name) = if obj_info.final_type
            || fn_info.kind == FunctionKind::Constructor
            || fn_info.kind == FunctionKind::Function
        {
            (obj_info.name.clone(), obj_info.name.clone())
        } else {
            let type_name = if fn_info.status == GStatus::Generate {
                obj_info.trait_name.clone()
            } else {
                format!("{}Manual", obj_info.trait_name)
            };
            (format!("prelude::{}", type_name), type_name)
        };
        let name = sym.full_rust_name().replace(&obj_info.name, &type_name);
        format!(
            "[{visible_type_name}::{fn_name}](crate::{name}::{fn_name})",
            name = name,
            visible_type_name = visible_type_name,
            fn_name = fn_info.codegen_name()
        )
    } else if let Some((record_info, fn_info)) = is_record_func {
        let sym = symbols.by_tid(record_info.type_id).unwrap(); // we are sure the object exists
        format!(
            "[{name}::{fn_name}](crate::{name}::{fn_name})",
            name = sym.full_rust_name(),
            fn_name = fn_info.codegen_name()
        )
    } else if let Some(fn_info) = is_globa_func {
        format!(
            "[{fn_name}()](crate::{fn_name})",
            fn_name = fn_info.codegen_name()
        )
    } else {
        format!("`{}`", name)
    }
}
