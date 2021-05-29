use super::gi_docgen;
use crate::{
    analysis::functions::Info,
    library::{FunctionKind, TypeId},
    nameutil, Env,
};
use log::{info, warn};
use once_cell::sync::Lazy;
use regex::{Captures, Regex};

const LANGUAGE_SEP_BEGIN: &str = "<!-- language=\"";
const LANGUAGE_SEP_END: &str = "\" -->";
const LANGUAGE_BLOCK_BEGIN: &str = "|[";
const LANGUAGE_BLOCK_END: &str = "\n]|";

pub fn reformat_doc(input: &str, env: &Env, in_type: Option<&TypeId>) -> String {
    code_blocks_transformation(input, env, in_type)
}

fn try_split<'a>(src: &'a str, needle: &str) -> (&'a str, Option<&'a str>) {
    match src.find(needle) {
        Some(pos) => (&src[..pos], Some(&src[pos + needle.len()..])),
        None => (src, None),
    }
}

fn code_blocks_transformation(mut input: &str, env: &Env, in_type: Option<&TypeId>) -> String {
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

fn format(input: &str, env: &Env, in_type: Option<&TypeId>) -> String {
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

fn replace_c_types(entry: &str, env: &Env, in_type: Option<&TypeId>) -> String {
    let out = FUNCTION.replace_all(entry, |caps: &Captures<'_>| {
        let name = &caps[3];
        find_function(name, env, in_type).unwrap_or_else(|| {
            info!("No function found for `{}()`", name);
            format!("`{}()`", name)
        })
    });

    let out = SYMBOL.replace_all(&out, |caps: &Captures<'_>| match &caps[2] {
        "TRUE" => "[`true`]".to_string(),
        "FALSE" => "[`false`]".to_string(),
        "NULL" => "[`None`]".to_string(),
        symbol_name => match &caps[1] {
            "%" => find_constant_or_variant(symbol_name, env, in_type),
            "#" => {
                let method_name = caps.get(3).map(|m| m.as_str().trim_start_matches('.'));
                find_method_or_type(symbol_name, method_name, env, in_type)
            }
            s => panic!("Unknown symbol prefix `{}`", s),
        }
        .unwrap_or_else(|| {
            info!("Symbol `{}{}` not found", &caps[1], symbol_name);

            format!("`{}`", symbol_name)
        }),
    });
    let out = GDK_GTK.replace_all(&out, |caps: &Captures<'_>| {
        find_type(&caps[2], env, in_type).unwrap_or_else(|| format!("`{}`", &caps[2]))
    });
    let out = TAGS.replace_all(&out, "`$0`");
    SPACES.replace_all(&out, " ").into_owned()
}

fn find_method_or_type(
    type_: &str,
    method_name: Option<&str>,
    env: &Env,
    in_type: Option<&TypeId>,
) -> Option<String> {
    let symbols = env.symbols.borrow();
    if let Some(method) = method_name {
        let is_signal = method.starts_with("::");
        let is_property = !is_signal && method.starts_with(':');
        if !is_signal && !is_property {
            if let Some((obj_info, fn_info)) = env.analysis.find_object_by_function(
                env,
                |o| o.c_type == type_,
                |f| f.name == method,
            ) {
                Some(gen_object_fn_doc_link(
                    obj_info,
                    fn_info,
                    env,
                    in_type,
                    &obj_info.name,
                ))
            } else if let Some((record_info, fn_info)) = env.analysis.find_record_by_function(
                env,
                |r| r.type_(&env.library).c_type == type_,
                |f| f.name == method,
            ) {
                Some(gen_record_fn_doc_link(
                    record_info.type_id,
                    fn_info,
                    env,
                    in_type,
                ))
            } else {
                warn!("Method `{}` of type `{}` not found", method, type_);
                None
            }
        } else {
            env.analysis
                .objects
                .values()
                .find(|o| o.c_type == type_)
                .map(|info| {
                    let sym = symbols.by_tid(info.type_id).unwrap(); // we are sure the object exists
                    let name = method.trim_start_matches(':');
                    if is_property {
                        gen_property_doc_link(&sym.full_rust_name(), name)
                    } else {
                        gen_signal_doc_link(&sym.full_rust_name(), name)
                    }
                })
        }
    } else {
        find_type(type_, env, in_type)
    }
}

fn find_constant_or_variant(symbol: &str, env: &Env, in_type: Option<&TypeId>) -> Option<String> {
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
        warn!("Constant/Flag variant/Enum member `{}` not found", symbol);
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
fn find_type(type_: &str, env: &Env, in_type: Option<&TypeId>) -> Option<String> {
    if IGNORED_C_TYPES.contains(&type_) {
        return None;
    }

    let symbols = env.symbols.borrow();

    let symbol = if let Some(obj) = env.analysis.objects.values().find(|o| o.c_type == type_) {
        symbols.by_tid(obj.type_id)
    } else if let Some(record) = env
        .analysis
        .records
        .values()
        .find(|r| r.type_(&env.library).c_type == type_)
    {
        symbols.by_tid(record.type_id)
    } else if let Some(enum_) = env
        .analysis
        .enumerations
        .iter()
        .find(|e| e.type_(&env.library).c_type == type_)
    {
        symbols.by_tid(enum_.type_id)
    } else if let Some(flag) = env
        .analysis
        .flags
        .iter()
        .find(|f| f.type_(&env.library).c_type == type_)
    {
        symbols.by_tid(flag.type_id)
    } else {
        warn!("Object/Interface/Record `{}` not found", type_);
        None
    };

    symbol.map_or_else(
        || {
            find_constant_or_variant(type_, env, in_type).map(|i| {
                warn!(
                    "`{}` was found to be a constant or enum-variant (`%`) but is instead prefixed with (`#`)",
                    type_
                );
                i
            })
        },
        |sym| Some(format!("[`{n}`][crate::{n}]", n = sym.full_rust_name())),
    )
}

// A list of function names that are ignored when warning about a "not found function"
const IGNORE_C_WARNING_FUNCS: [&str; 6] = [
    "g_object_unref",
    "g_object_ref",
    "g_free",
    "g_list_free",
    "g_strfreev",
    "printf",
];
/// Find a function in all the possible items, if not found return the original name surrounded with backsticks
/// A function can either be a struct/interface/record method, a global function or maybe a virtual function
fn find_function(name: &str, env: &Env, in_type: Option<&TypeId>) -> Option<String> {
    // if we can find the function in an object
    if let Some((obj_info, fn_info)) =
        env.analysis
            .find_object_by_function(env, |_| true, |f| f.glib_name == name)
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
            .find_record_by_function(env, |_| true, |f| f.glib_name == name)
    {
        Some(gen_record_fn_doc_link(
            record_info.type_id,
            fn_info,
            env,
            in_type,
        ))
    // or as a global function
    } else if let Some(fn_info) = env
        .analysis
        .find_global_function(env, |f| f.glib_name == name)
    {
        Some(fn_info.doc_link(None, None, false))
    } else {
        if !IGNORE_C_WARNING_FUNCS.contains(&name) {
            warn!("Function `{}` not found", name);
        }
        None
    }
}

pub(crate) fn gen_record_fn_doc_link(
    type_id: TypeId,
    fn_info: &Info,
    env: &Env,
    in_type: Option<&TypeId>,
) -> String {
    let symbols = env.symbols.borrow();
    let sym_name = symbols.by_tid(type_id).unwrap().full_rust_name();
    let is_self = in_type.map(|t| t == &type_id).unwrap_or(false);

    fn_info.doc_link(Some(&sym_name), None, is_self)
}

pub(crate) fn gen_object_fn_doc_link(
    obj_info: &crate::analysis::object::Info,
    fn_info: &Info,
    env: &Env,
    in_type: Option<&TypeId>,
    visible_name: &str,
) -> String {
    let symbols = env.symbols.borrow();
    let sym = symbols.by_tid(obj_info.type_id).unwrap();
    let is_self = in_type.map(|t| t == &obj_info.type_id).unwrap_or(false);

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
    in_type: Option<&TypeId>,
) -> String {
    let symbols = env.symbols.borrow();
    let sym = symbols.by_tid(type_id).unwrap().full_rust_name();
    let is_self = in_type.map(|t| t == &type_id).unwrap_or(false);
    let visible_sym = if is_self {
        "Self".to_string()
    } else {
        sym.clone()
    };

    format!(
        "[`{p}::{m}`][crate::{s}::{m}]",
        m = member_name,
        s = sym,
        p = visible_sym
    )
}

pub(crate) fn gen_const_doc_link(const_info: &crate::analysis::constants::Info) -> String {
    // for whatever reason constants are not part of the symbols list
    format!("[`{n}`][crate::{n}]", n = const_info.name)
}

pub(crate) fn gen_signal_doc_link(symbol: &str, signal: &str) -> String {
    format!("`signal::{}::{}`", symbol, signal)
}

pub(crate) fn gen_property_doc_link(symbol: &str, property: &str) -> String {
    format!("`property::{}::{}`", symbol, property)
}

pub(crate) fn gen_vfunc_doc_link(symbol: &str, vfunc: &str) -> String {
    format!("`vfunc::{}::{}`", symbol, vfunc)
}

pub(crate) fn gen_callback_doc_link(callback: &str) -> String {
    format!("`callback::{}", callback)
}

pub(crate) fn gen_alias_doc_link(alias: &str) -> String {
    format!("`alias::{}`", alias)
}

pub(crate) fn gen_symbol_doc_link(type_id: TypeId, env: &Env) -> String {
    let symbols = env.symbols.borrow();
    let sym = symbols.by_tid(type_id).unwrap();
    format!("[`{n}`][crate::{n}]", n = sym.full_rust_name())
}
