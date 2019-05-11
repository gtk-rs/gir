use lazy_static::lazy_static;
use regex::{Captures, Regex};

use crate::analysis::symbols;

const LANGUAGE_SEP_BEGIN: &str = "<!-- language=\"";
const LANGUAGE_SEP_END: &str = "\" -->";
const LANGUAGE_BLOCK_BEGIN: &str = "|[";
const LANGUAGE_BLOCK_END: &str = "\n]|";

pub fn reformat_doc(input: &str, symbols: &symbols::Info) -> String {
    code_blocks_transformation(input, symbols)
}

fn try_split<'a>(src: &'a str, needle: &str) -> (&'a str, Option<&'a str>) {
    match src.find(needle) {
        Some(pos) => (&src[..pos], Some(&src[pos + needle.len()..])),
        None => (src, None),
    }
}

fn code_blocks_transformation(mut input: &str, symbols: &symbols::Info) -> String {
    let mut out = String::with_capacity(input.len());

    loop {
        input = match try_split(input, LANGUAGE_BLOCK_BEGIN) {
            (before, Some(after)) => {
                out.push_str(&format(before, symbols));
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
                out.push_str(&format(before, symbols));
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

lazy_static! {
    static ref SYMBOL: Regex = Regex::new(r"(^|[^\\])[@#%]([\w]+\b)([:.]+[\w_-]+\b)?").unwrap();
    static ref FUNCTION: Regex = Regex::new(r"(\b[a-z0-9_]+)\(\)").unwrap();
    static ref GDK_GTK: Regex = Regex::new(r"G[dt]k[A-Z][\w]+\b").unwrap();
    static ref TAGS: Regex = Regex::new(r"<[\w/-]+>").unwrap();
    static ref SPACES: Regex = Regex::new(r"[ ][ ]+").unwrap();
}

fn format(mut input: &str, symbols: &symbols::Info) -> String {
    let mut ret = String::with_capacity(input.len());
    loop {
        let (before, after) = try_split(input, "`");
        ret.push_str(&replace_c_types(before, symbols));
        if let Some(after) = after {
            ret.push_str("`");
            let (before, after) = try_split(after, "`");
            // don't touch anything enclosed in backticks
            ret.push_str(before);
            if let Some(after) = after {
                ret.push_str("`");
                input = after;
            } else {
                return ret;
            }
        } else {
            return ret;
        }
    }
}

fn replace_c_types(entry: &str, symbols: &symbols::Info) -> String {
    let lookup = |s: &str| -> String {
        symbols
            .by_c_name(s)
            .map(symbols::Symbol::full_rust_name)
            .unwrap_or_else(|| s.into())
    };
    let out = SYMBOL.replace_all(entry, |caps: &Captures<'_>| {
        format!(
            "{}`{}{}`",
            &caps[1],
            lookup(&caps[2]),
            caps.get(3).map(|m| m.as_str()).unwrap_or("")
        )
    });
    let out = GDK_GTK.replace_all(&out, |caps: &Captures<'_>| {
        format!("`{}`", lookup(&caps[0]))
    });
    let out = FUNCTION.replace_all(&out, |caps: &Captures<'_>| {
        format!("`{}`", lookup(&caps[1]))
    });
    let out = TAGS.replace_all(&out, "`$0`");
    SPACES.replace_all(&out, " ").into_owned()
}
