use crate::analysis::symbols;
use once_cell::sync::Lazy;
use regex::{Captures, Match, Regex};

const LANGUAGE_SEP_BEGIN: &str = "<!-- language=\"";
const LANGUAGE_SEP_END: &str = "\" -->";
const LANGUAGE_BLOCK_BEGIN: &str = "|[";
const LANGUAGE_BLOCK_END: &str = "\n]|";

pub fn reformat_doc(input: &str, symbols: &symbols::Info, in_type: &str) -> String {
    code_blocks_transformation(input, symbols, in_type)
}

fn try_split<'a>(src: &'a str, needle: &str) -> (&'a str, Option<&'a str>) {
    match src.find(needle) {
        Some(pos) => (&src[..pos], Some(&src[pos + needle.len()..])),
        None => (src, None),
    }
}

fn code_blocks_transformation(mut input: &str, symbols: &symbols::Info, in_type: &str) -> String {
    let mut out = String::with_capacity(input.len());

    loop {
        input = match try_split(input, LANGUAGE_BLOCK_BEGIN) {
            (before, Some(after)) => {
                out.push_str(&format(before, symbols, in_type));
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
                out.push_str(&format(before, symbols, in_type));
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

fn format(mut input: &str, symbols: &symbols::Info, in_type: &str) -> String {
    let mut ret = String::with_capacity(input.len());
    loop {
        let (before, after) = try_split(input, "`");
        ret.push_str(&replace_c_types(before, symbols, in_type));
        if let Some(after) = after {
            ret.push('`');
            let (before, after) = try_split(after, "`");
            // don't touch anything enclosed in backticks
            ret.push_str(before);
            if let Some(after) = after {
                ret.push('`');
                input = after;
            } else {
                return ret;
            }
        } else {
            return ret;
        }
    }
}

static SYMBOL: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(^|[^\\])([@#%])(\w+\b)([:.]+[\w-]+\b)?").unwrap());
static FUNCTION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"([@#%])?(\w+\b[:.]+)?(\b[a-z0-9_]+)\(\)").unwrap());
static GDK_GTK: Lazy<Regex> = Lazy::new(|| Regex::new(r"G[dt]k[A-Z]\w+\b").unwrap());
static TAGS: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[\w/-]+>").unwrap());
static SPACES: Lazy<Regex> = Lazy::new(|| Regex::new(r"[ ]{2,}").unwrap());

fn replace_c_types(entry: &str, symbols: &symbols::Info, in_type: &str) -> String {
    let lookup = |s: &str| -> String {
        symbols
            .by_c_name(s)
            .map(symbols::Symbol::full_rust_name)
            .unwrap_or_else(|| s.into())
    };

    let out = FUNCTION.replace_all(entry, |caps: &Captures<'_>| {
        let name = &caps[3];
        let sym = symbols.by_c_name(name);

        if let Some(sym) = sym {
            if sym.owner_name() == Some(in_type) {
                // `#` or `%` symbols should probably have been `@` to denote
                // that it is a reference within the current type.
                format!("[`{f}()`][Self::{f}()]", f = sym.name())
            } else {
                match caps.get(1).as_ref().map(Match::as_str) {
                    // Catch invalid @ references that have a C symbol available but do not belong
                    // to the current type (and can hence not use `Self::`). For now generate XXX
                    // but with a valid global link so that the can be easily spotted in the code.
                    // assert_eq!(sym.owner_name(), Some(in_type));
                    Some("@") => format!(
                        "[`crate::{}()`] (XXX: @-reference does not belong to {}!)",
                        sym.full_rust_name(),
                        in_type,
                    ),
                    Some("#") | None => {
                        format!("[`{f}()`][crate::{f}()]", f = sym.full_rust_name())
                    }
                    Some("%") => panic!("% not allowed for {:?}", caps),
                    Some(c) => panic!("Unknown symbol reference {}", c),
                }
            }
        } else if let Some(typ) = caps.get(2) {
            let typ = typ.as_str();
            if typ == in_type {
                format!("[`{f}()`][Self::{f}()]", f = name)
            } else {
                format!("[`{t}{f}()`][crate::{t}{f}()]", t = typ, f = name)
            }
        } else {
            format!("`{}()`", name)
        }
    });

    let out = SYMBOL.replace_all(&out, |caps: &Captures<'_>| {
        let member = caps.get(4).map(|m| m.as_str()).unwrap_or("");
        let sym = symbols.by_c_name(&caps[3]);

        if let Some(sym) = sym {
            if sym.owner_name() == Some(in_type) {
                // `#` or `%` symbols should probably have been `@` to denote
                // that it is a reference within the current type.
                format!(
                    "{}[`{n}{m}`][Self::{n}{m}]",
                    &caps[1],
                    n = sym.name(),
                    m = member
                )
            } else {
                match &caps[2] {
                    // Catch invalid @ references that have a C symbol available but do not belong
                    // to the current type (and can hence not use `Self::`). For now generate XXX
                    // but with a valid global link so that the can be easily spotted in the code.
                    // assert_eq!(sym.owner_name(), Some(in_type));
                    "@" => format!(
                        "{}[`crate::{}{}`] (XXX: @-reference does not belong to {}!)",
                        &caps[1],
                        sym.full_rust_name(),
                        member,
                        in_type,
                    ),
                    "%" if sym.is_rust_prelude() => {
                        format!("{}[`{}{}`]", &caps[1], sym.full_rust_name(), member)
                    }
                    "#" | "%" => format!(
                        "{}[`{n}{m}`][crate::{n}{m}]",
                        &caps[1],
                        n = sym.full_rust_name(),
                        m = member
                    ),
                    c => panic!("Unknown symbol reference {}", c),
                }
            }
        } else {
            format!("{}`{}{}`", &caps[1], &caps[3], member)
        }
    });
    let out = GDK_GTK.replace_all(&out, |caps: &Captures<'_>| {
        format!("`{}`", lookup(&caps[0]))
    });
    let out = TAGS.replace_all(&out, "`$0`");
    SPACES.replace_all(&out, " ").into_owned()
}
