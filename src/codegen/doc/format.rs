use regex::{Captures, Regex};

const LANGUAGE_SEP_END : &'static str = "\" -->";
const LANGUAGE_BLOCK_BEGIN : &'static str = "|[<!-- language=\"";
const LANGUAGE_BLOCK_END : &'static str = "\n]|";

pub fn reformat_doc(input: &str) -> String {
    let out = code_blocks_transformation(input);
    replace_c_types(&out)
}

fn code_blocks_transformation(input: &str) -> String {
    let mut out = String::new();

    for entry in input.split(LANGUAGE_BLOCK_BEGIN) {
        if out.is_empty() {
            out.push_str(entry);
            continue;
        }
        out.push_str("```");
        let entry = get_language(entry, &mut out);
        out.push_str(&format!("{}", entry.replace(LANGUAGE_BLOCK_END, "\n```")))
    }
    out
}

fn get_language<'a>(entry: &'a str, out: &mut String) -> &'a str {
    if let Some(pos) = entry.find(LANGUAGE_SEP_END) {
        out.push_str(&entry[0..pos]);
        &entry[(pos + LANGUAGE_SEP_END.len())..]
    } else {
        out.push_str("\n");
        entry
    }
}

fn replace_c_types(entry: &str) -> String {
    if let Ok(reg) = Regex::new(r"Gtk[\w]*") {
        let mut out = vec!();

        for (num, part) in entry.split("```").into_iter().enumerate() {
            out.push(
                if num & 1 == 0 {
                    reg.replace_all(part, |caps: &Captures| {
                        match caps.at(0) {
                            Some(s) => format!("`{}`", &s[4..]),
                            None => String::new(),
                        }
                    })
                } else {
                    part.to_owned()
                }
            );
        }
        out.join("```")
    } else {
        entry.to_owned()
    }
}
