use super::defines::*;

// TODO: convert to macro with usage
// format!(indent!(5, "format:{}"), 6)
pub fn tabs(num: usize) -> String {
    format!("{:1$}", "", TAB_SIZE * num)
}

pub fn format_block(prefix: &str, suffix: &str, body: &[String]) -> Vec<String> {
    let mut v = Vec::new();
    if !prefix.is_empty() {
        v.push(prefix.into());
    }
    for s in body.iter() {
        let s = format!("{TAB}{s}");
        v.push(s);
    }
    if !suffix.is_empty() {
        v.push(suffix.into());
    }
    v
}

pub fn format_block_one_line(
    prefix: &str,
    suffix: &str,
    body: &[String],
    outer_separator: &str,
    inner_separator: &str,
) -> String {
    let mut s = format!("{prefix}{outer_separator}");
    let mut first = true;
    for s_ in body {
        if first {
            first = false;
            s = s + s_;
        } else {
            s = s + inner_separator + s_;
        }
    }
    s + outer_separator + suffix
}

pub fn format_block_smart(
    prefix: &str,
    suffix: &str,
    body: &[String],
    outer_separator: &str,
    inner_separator: &str,
) -> Vec<String> {
    format_block_smart_width(
        prefix,
        suffix,
        body,
        outer_separator,
        inner_separator,
        MAX_TEXT_WIDTH,
    )
}

pub fn format_block_smart_width(
    prefix: &str,
    suffix: &str,
    body: &[String],
    outer_separator: &str,
    inner_separator: &str,
    max_width: usize,
) -> Vec<String> {
    let outer_len = prefix.len() + suffix.len() + 2 * outer_separator.len();
    let mut inner_len = inner_separator.len() * (body.len() - 1);
    // TODO: change to sum()
    for s in body {
        inner_len += s.len();
    }
    if (outer_len + inner_len) > max_width {
        format_block(prefix, suffix, body)
    } else {
        let s = format_block_one_line(prefix, suffix, body, outer_separator, inner_separator);
        vec![s]
    }
}

pub fn comment_block(body: &[String]) -> Vec<String> {
    body.iter().map(|s| format!("//{s}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tabs() {
        assert_eq!(tabs(0), "");
        assert_eq!(tabs(1), TAB);
        assert_eq!(tabs(2), format!("{TAB}{TAB}"));
    }

    #[test]
    fn test_format_block() {
        let body = vec!["0 => 1,".into(), "1 => 0,".into()];
        let actual = format_block("match a {", "}", &body);
        let expected = ["match a {", "    0 => 1,", "    1 => 0,", "}"];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_block_smart_width_one_line_outer_separator() {
        let body = vec!["f()".into()];
        let actual = format_block_smart_width("unsafe {", "}", &body, " ", "", 14);
        let expected = ["unsafe { f() }"];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_block_smart_width_many_lines_outer_separator() {
        let body = vec!["f()".into()];
        let actual = format_block_smart_width("unsafe {", "}", &body, " ", "", 13);
        let expected = ["unsafe {", "    f()", "}"];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_format_block_smart_one_line_inner_separator() {
        let body = vec!["a: &str".into(), "b: &str".into()];
        let actual = format_block_smart("f(", ")", &body, "", ", ");
        let expected = ["f(a: &str, b: &str)"];
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_comment_block() {
        let body = vec!["f(a,".into(), "  b)".into()];
        let actual = comment_block(&body);
        let expected = ["//f(a,", "//  b)"];
        assert_eq!(actual, expected);
    }
}
