use std::ascii::AsciiExt;
use regex::{Captures, Regex};

const LANGUAGE_SEP_BEGIN : &'static str = "<!-- language=\"";
const LANGUAGE_SEP_END : &'static str = "\" -->";
const LANGUAGE_BLOCK_BEGIN : &'static str = "|[";
const LANGUAGE_BLOCK_END : &'static str = "\n]|";

pub fn reformat_doc(input: &str, namespace_name: &str) -> String {
    code_blocks_transformation(input, namespace_name)
}

fn try_split<'a>(src: &'a str, needle: &str) -> (&'a str, Option<&'a str>) {
    match src.find(needle) {
        Some(pos) => (&src[..pos], Some(&src[pos + needle.len()..])),
        None => (src, None),
    }
}

fn code_blocks_transformation(mut input: &str, namespace_name: &str) -> String {
    let mut out = String::new();

    loop {
        input = match try_split(input, LANGUAGE_BLOCK_BEGIN) {
            (before, Some(after)) => {
                out.push_str(&replace_c_types(before, namespace_name));
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
                out.push_str(&replace_c_types(before, namespace_name));
                return out
            }
        };
    }
}

lazy_static! {
    static ref REG : Regex = Regex::new(r"#?(G[dt]k)([\w]*)").unwrap();
    static ref REG2 : Regex = Regex::new(r"@(\w*)").unwrap();
}

fn get_language<'a>(entry: &'a str, out: &mut String) -> &'a str {
    if let (_, Some(after)) = try_split(entry, LANGUAGE_SEP_BEGIN) {
        if let (before, Some(after)) = try_split(after, LANGUAGE_SEP_END) {
            out.push_str(&format!("```{}", before));
            return after
        }
    }
    out.push_str("```text");
    entry
}

fn replace_c_types(entry: &str, namespace_name: &str) -> String {
    let out = &REG.replace_all(entry, |caps: &Captures| {
        if caps[1].eq_ignore_ascii_case(namespace_name) {
            format!("`{}`", &caps[2])
        } else {
            format!("`{}::{}`", &caps[1].to_lowercase(), &caps[2])
        }
    });
    REG2.replace_all(out, "`$1`")
}
