use regex::{Captures, Regex};

const LANGUAGE_SEP_BEGIN : &'static str = "<!-- language=\"";
const LANGUAGE_SEP_END : &'static str = "\" -->";
const LANGUAGE_BLOCK_BEGIN : &'static str = "|[";
const LANGUAGE_BLOCK_END : &'static str = "\n]|";

lazy_static! {
    static ref REG : Regex = Regex::new(r"#?(G[dt]k)([\w]*)").unwrap();
    static ref REG2 : Regex = Regex::new(r"@(\w*)").unwrap();
}

pub fn reformat_doc(input: &str) -> String {
    code_blocks_transformation(input)
}

fn try_split<'a>(src: &'a str, needle: &str) -> (&'a str, Option<&'a str>) {
    match src.find(needle) {
        Some(pos) => (&src[..pos], Some(&src[pos + needle.len()..])),
        None => (src, None),
    }
}

fn code_blocks_transformation(mut input: &str) -> String {
    let mut out = String::new();

    loop {
        input = match try_split(input, LANGUAGE_BLOCK_BEGIN) {
            (before, Some(after)) => {
                out.push_str(&replace_c_types(before));
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
                out.push_str(&replace_c_types(before));
                return out
            }
        };
    }
}

fn get_language<'a>(entry: &'a str, out: &mut String) -> &'a str {
    if let Some(pos) = entry.find(LANGUAGE_SEP_BEGIN) {
        let entry = &entry[pos + LANGUAGE_SEP_BEGIN.len()..];
        if let Some(pos) = entry.find(LANGUAGE_SEP_END) {
            out.push_str(&format!("```{}", &entry[0..pos]));
            return &entry[(pos + LANGUAGE_SEP_END.len())..]
        }
    }
    out.push_str("```");
    entry
}

fn replace_c_types(entry: &str) -> String {
    let out = &REG.replace_all(entry, |caps: &Captures| {
        format!("`{}::{}`", &caps[1].to_lowercase(), &caps[2])
    });
    REG2.replace_all(out, "`$1`")
}
