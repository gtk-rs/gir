use analysis::symbols;
use regex::{Captures, Regex};
use super::TypeReferences;

const LANGUAGE_SEP_BEGIN : &'static str = "<!-- language=\"";
const LANGUAGE_SEP_END : &'static str = "\" -->";
const LANGUAGE_BLOCK_BEGIN : &'static str = "|[";
const LANGUAGE_BLOCK_END : &'static str = "\n]|";

pub fn reformat_doc(input: &str, symbols: &symbols::Info, refs: &TypeReferences) -> String {
    code_blocks_transformation(input, symbols, refs)
}

fn try_split<'a>(src: &'a str, needle: &str) -> (&'a str, Option<&'a str>) {
    match src.find(needle) {
        Some(pos) => (&src[..pos], Some(&src[pos + needle.len()..])),
        None => (src, None),
    }
}

fn code_blocks_transformation(mut input: &str, symbols: &symbols::Info,
                              refs: &TypeReferences) -> String {
    let mut out = String::with_capacity(input.len());

    loop {
        input = match try_split(input, LANGUAGE_BLOCK_BEGIN) {
            (before, Some(after)) => {
                out.push_str(&format(before, symbols, refs));
                if let (before, Some(after)) = try_split(get_language(after, &mut out),
                                                         LANGUAGE_BLOCK_END) {
                    out.push_str(before);
                    out.push_str("\n```");
                    after
                } else {
                    after
                }
            }
            (before, None) => {
                out.push_str(&format(before, symbols, refs));
                return out
            }
        };
    }
}

fn get_language<'a>(entry: &'a str, out: &mut String) -> &'a str {
    if let (_, Some(after)) = try_split(entry, LANGUAGE_SEP_BEGIN) {
        if let (before, Some(after)) = try_split(after, LANGUAGE_SEP_END) {
            out.push_str(&format!("\n```{}", before));
            return after
        }
    }
    out.push_str("\n```text");
    entry
}

lazy_static! {
    static ref SYMBOL: Regex = Regex::new(r"(^|[^\\])[@#%]([\w]+\b)([:.]+[\w_-]+\b)?") .unwrap();
    static ref FUNCTION: Regex = Regex::new(r"(\b[a-z0-9_]+)\(\)") .unwrap();
    static ref GDK_GTK: Regex = Regex::new(r"G[dt]k[A-Z][\w]+\b").unwrap();
    static ref TAGS: Regex = Regex::new(r"<[\w/-]+>").unwrap();
    static ref SPACES: Regex = Regex::new(r"[ ][ ]+").unwrap();
}

fn format(mut input: &str, symbols: &symbols::Info, refs: &TypeReferences) -> String {
    let mut ret = String::with_capacity(input.len());
    loop {
        let (before, after) = try_split(input, "`");
        ret.push_str(&replace_c_types(before, symbols, refs));
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

fn to_url(type_name: &str, refs: &TypeReferences) -> Option<String> {
    let ty = type_name.split(":");

    if let Some(t) = refs.get_type(&ty.last().unwrap().to_owned()) {
        Some(format!("{}.{}.html", t.ty, t.name))
    } else {
        None
    }
}

fn replace_c_types(entry: &str, symbols: &symbols::Info, refs: &TypeReferences) -> String {
    let lookup = |s: &str| -> String {
        symbols.by_c_name(s)
            .map(|s| s.full_rust_name())
            .unwrap_or(s.into())
    };
    let out = SYMBOL.replace_all(entry, |caps: &Captures| {
        let after = lookup(&caps[2]);
        if let Some(url) = to_url(&after, refs) {
            format!("{}[`{}{}`]({})", &caps[1], after, caps.at(3).unwrap_or(""), url)
        } else {
            format!("{}`{}{}`", &caps[1], after, caps.at(3).unwrap_or(""))
        }
    });
    let out = FUNCTION.replace_all(&out, |caps: &Captures| {
        let after = lookup(&caps[1]);
        if let Some(url) = to_url(&after, refs) {
            format!("[`{}`]({})", after, url)
        } else {
            format!("`{}`", after)
        }
    });
    let out = GDK_GTK.replace_all(&out, |caps: &Captures| {
        let after = lookup(&caps[0]);
        if let Some(url) = to_url(&after, refs) {
            format!("[`{}`]({})", after, url)
        } else {
            format!("`{}`", after)
        }
    });
    let out = TAGS.replace_all(&out, "`$0`");
    SPACES.replace_all(&out, " ")
}
