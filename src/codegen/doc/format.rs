#![allow(clippy::manual_map)]
use std::fmt::Write;

use log::{info, warn};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};

use super::{gi_docgen, LocationInObject};
use crate::{
    analysis::functions::Info,
    library::{FunctionKind, TypeId},
    nameutil, Env,
};

const LANGUAGE_SEP_BEGIN: &str = "<!-- language=\"";
const LANGUAGE_SEP_END: &str = "\" -->";
const LANGUAGE_BLOCK_BEGIN: &str = "|[";
const LANGUAGE_BLOCK_END: &str = "\n]|";

// A list of function names that are ignored when warning about a "not found
// function"
const IGNORE_C_WARNING_FUNCS: [&str; 6] = [
    "g_object_unref",
    "g_object_ref",
    "g_free",
    "g_list_free",
    "g_strfreev",
    "printf",
];

pub fn reformat_doc(
    input: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> String {
    code_blocks_transformation(input, env, in_type)
}

fn try_split<'a>(src: &'a str, needle: &str) -> (&'a str, Option<&'a str>) {
    match src.find(needle) {
        Some(pos) => (&src[..pos], Some(&src[pos + needle.len()..])),
        None => (src, None),
    }
}

fn code_blocks_transformation(
    mut input: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> String {
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
            if !["text", "rust"].contains(&before) {
                write!(out, "\n\n**⚠️ The following code is in {before} ⚠️**").unwrap();
            }
            write!(out, "\n\n```{before}").unwrap();
            return after;
        }
    }
    out.push_str("\n```text");
    entry
}

// try to get the language if any is defined or fallback to text
fn get_markdown_language(input: &str) -> (&str, &str) {
    let (lang, after) = if let Some((lang, after)) = input.split_once('\n') {
        let lang = if lang.is_empty() { None } else { Some(lang) };
        (lang, after)
    } else {
        (None, input)
    };
    (lang.unwrap_or("text"), after)
}

// Re-format codeblocks & replaces the C types and GI-docgen with proper links
fn format(
    mut input: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> String {
    let mut ret = String::with_capacity(input.len());
    loop {
        input = match try_split(input, "```") {
            (before, Some(after)) => {
                // if we are inside a codeblock
                ret.push_str(&replace_symbols(before, env, in_type));

                let (lang, after) = get_markdown_language(after);
                if !["text", "rust", "xml", "css", "json", "html"].contains(&lang)
                    && after.lines().count() > 1
                {
                    write!(ret, "**⚠️ The following code is in {lang} ⚠️**\n\n").unwrap();
                }
                writeln!(ret, "```{lang}").unwrap();

                if let (before, Some(after)) = try_split(after, "```") {
                    ret.push_str(before);
                    ret.push_str("```");
                    after
                } else {
                    after
                }
            }
            (before, None) => {
                ret.push_str(&replace_symbols(before, env, in_type));
                return ret;
            }
        }
    }
}

fn replace_symbols(
    input: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> String {
    if env.config.use_gi_docgen {
        let out = gi_docgen::replace_c_types(input, env, in_type);
        let out = GI_DOCGEN_SYMBOL.replace_all(&out, |caps: &Captures<'_>| match &caps[2] {
            "TRUE" => "[`true`]".to_string(),
            "FALSE" => "[`false`]".to_string(),
            "NULL" => "[`None`]".to_string(),
            symbol_name => match &caps[1] {
                // Opt-in only for the %SYMBOLS, @/# causes breakages
                "%" => find_constant_or_variant_wrapper(symbol_name, env, in_type),
                s => panic!("Unknown symbol prefix `{s}`"),
            },
        });
        let out = GDK_GTK.replace_all(&out, |caps: &Captures<'_>| {
            find_type(&caps[2], env).unwrap_or_else(|| format!("`{}`", &caps[2]))
        });

        out.to_string()
    } else {
        replace_c_types(input, env, in_type)
    }
}

static SYMBOL: Lazy<Regex> = Lazy::new(|| Regex::new(r"([@#%])(\w+\b)([:.]+[\w-]+\b)?").unwrap());
static GI_DOCGEN_SYMBOL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([%])(\w+\b)([:.]+[\w-]+\b)?").unwrap());
static FUNCTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([@#%])?(\w+\b[:.]+)?(\b[a-z0-9_]+)\(\)").unwrap());
// **note**
// The optional . at the end is to make the regex more relaxed for some weird
// broken cases on gtk3's docs it doesn't hurt other docs so please don't drop
// it
static GDK_GTK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"`([^\(:])?((G[dts]k|Pango|cairo_|graphene_|Adw|Hdy|GtkSource)\w+\b)(\.)?`")
        .unwrap()
});
static TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[\w/-]+>").unwrap());
static SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ ]{2,}").unwrap());

fn replace_c_types(
    entry: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> String {
    let out = FUNCTION.replace_all(entry, |caps: &Captures<'_>| {
        let name = &caps[3];
        find_method_or_function_by_ctype(None, name, env, in_type).unwrap_or_else(|| {
            if !IGNORE_C_WARNING_FUNCS.contains(&name) {
                info!("No function found for `{}()`", name);
            }
            format!("`{}{}()`", caps.get(2).map_or("", |m| m.as_str()), name)
        })
    });

    let out = SYMBOL.replace_all(&out, |caps: &Captures<'_>| match &caps[2] {
        "TRUE" => "[`true`]".to_string(),
        "FALSE" => "[`false`]".to_string(),
        "NULL" => "[`None`]".to_string(),
        symbol_name => match &caps[1] {
            "%" => find_constant_or_variant_wrapper(symbol_name, env, in_type),
            "#" => {
                if let Some(member_path) = caps.get(3).map(|m| m.as_str()) {
                    let method_name = member_path.trim_start_matches('.');
                    find_member(symbol_name, method_name, env, in_type).unwrap_or_else(|| {
                        info!("`#{}` not found as method", symbol_name);
                        format!("`{symbol_name}{member_path}`")
                    })
                } else if let Some(type_) = find_type(symbol_name, env) {
                    type_
                } else if let Some(constant_or_variant) =
                    find_constant_or_variant(symbol_name, env, in_type)
                {
                    warn!(
                        "`{}` matches a constant/variant and should use `%` prefix instead of `#`",
                        symbol_name
                    );
                    constant_or_variant
                } else {
                    info!("Type `#{}` not found", symbol_name);
                    format!("`{symbol_name}`")
                }
            }
            "@" => {
                // XXX: Theoretically this code should check if the resulting
                // symbol truly belongs to `in_type`!
                if let Some(type_) = find_type(symbol_name, env) {
                    warn!(
                        "`{}` matches a type and should use `#` prefix instead of `%`",
                        symbol_name
                    );
                    type_
                } else if let Some(constant_or_variant) =
                    find_constant_or_variant(symbol_name, env, in_type)
                {
                    constant_or_variant
                } else if let Some(function) =
                    find_method_or_function_by_ctype(None, symbol_name, env, in_type)
                {
                    function
                } else {
                    // `@` is often used to refer to fields and function parameters.
                    format!("`{symbol_name}`")
                }
            }
            s => panic!("Unknown symbol prefix `{s}`"),
        },
    });
    let out = GDK_GTK.replace_all(&out, |caps: &Captures<'_>| {
        find_type(&caps[2], env).unwrap_or_else(|| format!("`{}`", &caps[2]))
    });
    let out = TAGS.replace_all(&out, "`$0`");
    SPACES.replace_all(&out, " ").into_owned()
}

/// Wrapper around [`find_constant_or_variant`] that fallbacks to returning
/// the `symbol_name`
fn find_constant_or_variant_wrapper(
    symbol_name: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> String {
    find_constant_or_variant(symbol_name, env, in_type).unwrap_or_else(|| {
        info!("Constant or variant `%{}` not found", symbol_name);
        format!("`{symbol_name}`")
    })
}

fn find_member(
    type_: &str,
    method_name: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> Option<String> {
    let symbols = env.symbols.borrow();
    let is_signal = method_name.starts_with("::");
    let is_property = !is_signal && method_name.starts_with(':');
    if !is_signal && !is_property {
        find_method_or_function_by_ctype(Some(type_), method_name, env, in_type)
    } else {
        env.analysis
            .objects
            .values()
            .find(|o| o.c_type == type_)
            .map(|info| {
                let sym = symbols.by_tid(info.type_id).unwrap(); // we are sure the object exists
                let name = method_name.trim_start_matches(':');
                if is_property {
                    gen_property_doc_link(&sym.full_rust_name(), name)
                } else {
                    gen_signal_doc_link(&sym.full_rust_name(), name)
                }
            })
    }
}

fn find_constant_or_variant(
    symbol: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> Option<String> {
    if let Some((flag_info, member_info)) = env.analysis.flags.iter().find_map(|f| {
        f.type_(&env.library)
            .members
            .iter()
            .find(|m| m.c_identifier == symbol && !m.status.ignored())
            .map(|m| (f, m))
    }) {
        Some(gen_member_doc_link(
            flag_info.type_id,
            &nameutil::bitfield_member_name(&member_info.name),
            env,
            in_type,
        ))
    } else if let Some((enum_info, member_info)) = env.analysis.enumerations.iter().find_map(|e| {
        e.type_(&env.library)
            .members
            .iter()
            .find(|m| m.c_identifier == symbol && !m.status.ignored())
            .map(|m| (e, m))
    }) {
        Some(gen_member_doc_link(
            enum_info.type_id,
            &nameutil::enum_member_name(&member_info.name),
            env,
            in_type,
        ))
    } else if let Some(const_info) = env
        .analysis
        .constants
        .iter()
        .find(|c| c.glib_name == symbol)
    {
        Some(gen_const_doc_link(const_info))
    } else {
        None
    }
}

// A list of types that are automatically ignored by the `find_type` function
const IGNORED_C_TYPES: [&str; 6] = [
    "gconstpointer",
    "guint16",
    "guint",
    "gunicode",
    "gchararray",
    "GList",
];
/// either an object/interface, record, enum or a flag
fn find_type(type_: &str, env: &Env) -> Option<String> {
    if IGNORED_C_TYPES.contains(&type_) {
        return None;
    }

    let type_id = if let Some(obj) = env.analysis.objects.values().find(|o| o.c_type == type_) {
        Some(obj.type_id)
    } else if let Some(record) = env
        .analysis
        .records
        .values()
        .find(|r| r.type_(&env.library).c_type == type_)
    {
        Some(record.type_id)
    } else if let Some(enum_) = env
        .analysis
        .enumerations
        .iter()
        .find(|e| e.type_(&env.library).c_type == type_)
    {
        Some(enum_.type_id)
    } else if let Some(flag) = env
        .analysis
        .flags
        .iter()
        .find(|f| f.type_(&env.library).c_type == type_)
    {
        Some(flag.type_id)
    } else {
        None
    };

    type_id.map(|ty| gen_symbol_doc_link(ty, env))
}

fn find_method_or_function_by_ctype(
    c_type: Option<&str>,
    name: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> Option<String> {
    find_method_or_function(
        name,
        env,
        in_type,
        |f| f.glib_name == name,
        |o| c_type.map_or(true, |t| o.c_type == t),
        |r| c_type.map_or(true, |t| r.type_(&env.library).c_type == t),
        |r| c_type.map_or(true, |t| r.type_(&env.library).c_type == t),
        |r| c_type.map_or(true, |t| r.type_(&env.library).c_type == t),
        c_type.map_or(false, |t| t.ends_with("Class")),
    )
}

/// Find a function in all the possible items, if not found return the original
/// name surrounded with backticks. A function can either be a
/// struct/interface/record method, a global function or maybe a virtual
/// function
///
/// This function is generic so it can be de-duplicated between a
/// - [`find_method_or_function_by_ctype()`] where the object/records are looked
///   by their C name
/// - [`gi_docgen::find_method_or_function_by_name()`] where the object/records
///   are looked by their name
pub(crate) fn find_method_or_function(
    name: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
    search_fn: impl Fn(&crate::analysis::functions::Info) -> bool + Copy,
    search_obj: impl Fn(&crate::analysis::object::Info) -> bool + Copy,
    search_record: impl Fn(&crate::analysis::record::Info) -> bool + Copy,
    search_enum: impl Fn(&crate::analysis::enums::Info) -> bool + Copy,
    search_flag: impl Fn(&crate::analysis::flags::Info) -> bool + Copy,
    is_class_method: bool,
) -> Option<String> {
    if is_class_method {
        info!("Class methods are not supported yet `{}`", name);
        return None;
    }

    // if we can find the function in an object
    if let Some((obj_info, fn_info)) = env
        .analysis
        .find_object_by_function(env, search_obj, search_fn)
    {
        Some(gen_object_fn_doc_link(
            obj_info,
            fn_info,
            env,
            in_type,
            &obj_info.name,
        ))
    // or in a record
    } else if let Some((record_info, fn_info)) =
        env.analysis
            .find_record_by_function(env, search_record, search_fn)
    {
        Some(gen_type_fn_doc_link(
            record_info.type_id,
            fn_info,
            env,
            in_type,
        ))
    } else if let Some((enum_info, fn_info)) =
        env.analysis
            .find_enum_by_function(env, search_enum, search_fn)
    {
        Some(gen_type_fn_doc_link(
            enum_info.type_id,
            fn_info,
            env,
            in_type,
        ))
    } else if let Some((flag_info, fn_info)) =
        env.analysis
            .find_flag_by_function(env, search_flag, search_fn)
    {
        Some(gen_type_fn_doc_link(
            flag_info.type_id,
            fn_info,
            env,
            in_type,
        ))
    // or as a global function
    } else if let Some(fn_info) = env.analysis.find_global_function(env, search_fn) {
        Some(fn_info.doc_link(None, None, false))
    } else {
        None
    }
}

pub(crate) fn gen_type_fn_doc_link(
    type_id: TypeId,
    fn_info: &Info,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> String {
    let symbols = env.symbols.borrow();
    let sym_name = symbols.by_tid(type_id).unwrap().full_rust_name();
    let is_self = in_type == Some((&type_id, None));

    fn_info.doc_link(Some(&sym_name), None, is_self)
}

pub(crate) fn gen_object_fn_doc_link(
    obj_info: &crate::analysis::object::Info,
    fn_info: &Info,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
    visible_name: &str,
) -> String {
    let symbols = env.symbols.borrow();
    let sym = symbols.by_tid(obj_info.type_id).unwrap();
    let is_self = in_type == Some((&obj_info.type_id, Some(obj_info.function_location(fn_info))));

    if fn_info.kind == FunctionKind::Method {
        let (type_name, visible_type_name) = obj_info.generate_doc_link_info(fn_info);

        fn_info.doc_link(
            Some(&sym.full_rust_name().replace(visible_name, &type_name)),
            Some(&visible_type_name),
            is_self,
        )
    } else {
        fn_info.doc_link(Some(&sym.full_rust_name()), None, is_self)
    }
}

// Helper function to generate a doc link for an enum member/bitfield variant
pub(crate) fn gen_member_doc_link(
    type_id: TypeId,
    member_name: &str,
    env: &Env,
    in_type: Option<(&TypeId, Option<LocationInObject>)>,
) -> String {
    let symbols = env.symbols.borrow();
    let sym = symbols.by_tid(type_id).unwrap().full_rust_name();
    let is_self = in_type == Some((&type_id, None));

    if is_self {
        format!("[`{member_name}`][Self::{member_name}]")
    } else {
        format!("[`{sym}::{member_name}`][crate::{sym}::{member_name}]")
    }
}

pub(crate) fn gen_const_doc_link(const_info: &crate::analysis::constants::Info) -> String {
    // for whatever reason constants are not part of the symbols list
    format!("[`{n}`][crate::{n}]", n = const_info.name)
}

pub(crate) fn gen_signal_doc_link(symbol: &str, signal: &str) -> String {
    format!("[`{signal}`][struct@crate::{symbol}#{signal}]")
}

pub(crate) fn gen_property_doc_link(symbol: &str, property: &str) -> String {
    format!("[`{property}`][struct@crate::{symbol}#{property}]")
}

pub(crate) fn gen_vfunc_doc_link(symbol: &str, vfunc: &str) -> String {
    format!("`vfunc::{symbol}::{vfunc}`")
}

pub(crate) fn gen_callback_doc_link(callback: &str) -> String {
    format!("`callback::{callback}")
}

pub(crate) fn gen_alias_doc_link(alias: &str) -> String {
    format!("`alias::{alias}`")
}

pub(crate) fn gen_symbol_doc_link(type_id: TypeId, env: &Env) -> String {
    let symbols = env.symbols.borrow();
    let sym = symbols.by_tid(type_id).unwrap();
    // Workaround the case of glib::Variant being a derive macro and a struct
    if sym.name() == "Variant" && (sym.crate_name().is_none() || sym.crate_name() == Some("glib")) {
        format!("[`{n}`][struct@crate::{n}]", n = sym.full_rust_name())
    } else {
        format!("[`{n}`][crate::{n}]", n = sym.full_rust_name())
    }
}
