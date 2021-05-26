use super::gi_docgen;
use crate::{nameutil, Env};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};

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
    let out = replace_c_types(input, env, in_type);
    let out = gi_docgen::replace_c_types(&out, env, in_type);
    // this has to be done after gi_docgen replaced the various types it knows as it uses `@` in it's linking format
    let out = PARAM_SYMBOL.replace_all(&out, |caps: &Captures<'_>| format!("`{}`", &caps[2]));
    ret.push_str(&out);
    ret
}

static SYMBOL: Lazy<Regex> = Lazy::new(|| Regex::new(r"([#%])(\w+\b)([:.]+[\w-]+\b)?").unwrap());
static PARAM_SYMBOL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([@])(\w+\b)([:.]+[\w-]+\b)?").unwrap());
static FUNCTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([@#%])?(\w+\b[:.]+)?(\b[a-z0-9_]+)\(\)").unwrap());
// **note**
// The optional . at the end is to make the regex more relaxed for some weird broken cases on gtk3's docs
// it doesn't hurt other docs so please don't drop it
static GDK_GTK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"`([^\(:])?((G[dts]k|Pango)\w+\b)(\.)?`").unwrap());
static TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[\w/-]+>").unwrap());
static SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ ]{2,}").unwrap());

fn replace_c_types(entry: &str, env: &Env, _in_type: &str) -> String {
    let out = FUNCTION.replace_all(entry, |caps: &Captures<'_>| {
        let name = &caps[3];
        find_function(name, env)
    });

    let out = SYMBOL.replace_all(&out, |caps: &Captures<'_>| {
        match &caps[2] {
            "TRUE" => "[`true`]".to_string(),
            "FALSE" => "[`false`]".to_string(),
            "NULL" => "[`None`]".to_string(),
            symbol_name => {
                if &caps[1] == "%" {
                    find_constant_or_variant(symbol_name, env)
                } else {
                    let method_name = caps.get(3).map(|m| m.as_str().trim_start_matches('.'));
                    // would be #
                    find_type_by_name(symbol_name, method_name, env)
                }
            }
        }
    });
    let out = GDK_GTK.replace_all(&out, |caps: &Captures<'_>| find_struct(&caps[2], env));
    let out = TAGS.replace_all(&out, "`$0`");
    SPACES.replace_all(&out, " ").into_owned()
}

fn find_type_by_name(symbol: &str, method_name: Option<&str>, env: &Env) -> String {
    let symbols = env.symbols.borrow();
    if let Some(method) = method_name {
        if let Some((obj_info, fn_info)) = env.analysis.find_object_by_function(
            env,
            |o| o.full_name == symbol,
            |f| f.name == method,
        ) {
            let sym = symbols.by_tid(obj_info.type_id).unwrap(); // we are sure the object exists
            let (type_name, visible_type_name) = obj_info.generate_doc_link_info(fn_info);
            let name = sym.full_rust_name().replace(&obj_info.name, &type_name);

            fn_info.doc_link(Some(&name), Some(&visible_type_name))
        } else if let Some((record_info, fn_info)) = env.analysis.find_record_by_function(
            env,
            |r| r.type_(&env.library).c_type == symbol,
            |f| f.name == method,
        ) {
            let sym_name = symbols
                .by_tid(record_info.type_id)
                .unwrap()
                .full_rust_name(); // we are sure the object exists
            fn_info.doc_link(Some(&sym_name), None)
        } else {
            format!("`{}::{}`", symbol, method)
        }
    } else {
        find_struct(symbol, env)
    }
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
        format!("[`{name}`][crate::{name}]", name = const_info.name)
    } else if let Some((flag_info, member_info)) = env.analysis.flags.iter().find_map(|f| {
        f.type_(&env.library)
            .members
            .iter()
            .find(|m| m.c_identifier == symbol)
            .map(|m| (f, m))
    }) {
        let sym = symbols.by_tid(flag_info.type_id).unwrap();
        format!(
            "[`{flag_name}::{member_name}`][crate::{parent}{member_name}]",
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
            "[`{enum_name}::{member}`][crate::{parent}{member}]",
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

    let symbol = if let Some(obj) = env.analysis.objects.values().find(|o| o.c_type == name) {
        symbols.by_tid(obj.type_id)
    } else if let Some(record) = env
        .analysis
        .records
        .values()
        .find(|r| r.type_(&env.library).c_type == name)
    {
        symbols.by_tid(record.type_id)
    } else {
        None
    };
    symbol
        .map(|sym| format!("[`{name}`][crate::{name}]", name = sym.full_rust_name()))
        .unwrap_or_else(|| name.to_string())
}

/// Find a function in all the possible items, if not found return the original name surrounded with backsticks
/// A function can either be a struct/interface/record method, a global function or maybe a virtual function
fn find_function(name: &str, env: &Env) -> String {
    let symbols = env.symbols.borrow();
    // if we can find the function in an object
    if let Some((obj_info, fn_info)) =
        env.analysis
            .find_object_by_function(env, |_| true, |f| f.glib_name == name)
    {
        let sym = symbols.by_tid(obj_info.type_id).unwrap(); // we are sure the object exists
        let (type_name, visible_type_name) = obj_info.generate_doc_link_info(fn_info);

        let name = sym.full_rust_name().replace(&obj_info.name, &type_name);
        fn_info.doc_link(Some(&name), Some(&visible_type_name))
    // or in a record
    } else if let Some((record_info, fn_info)) =
        env.analysis
            .find_record_by_function(env, |_| true, |f| f.glib_name == name)
    {
        let sym_name = symbols
            .by_tid(record_info.type_id)
            .unwrap()
            .full_rust_name(); // we are sure the object exists
        fn_info.doc_link(Some(&sym_name), None)
    // or as a global function
    } else if let Some(fn_info) = env
        .analysis
        .find_global_function(env, |f| f.glib_name == name)
    {
        format!(
            "[`{fn_name}()`][crate::{fn_name}()]",
            fn_name = fn_info.codegen_name()
        )
    } else {
        format!("`{}()`", name)
    }
}
