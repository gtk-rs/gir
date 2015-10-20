use super::defines::*;

//TODO: convert to macro with usage
//format!(indent!(5, "format:{}"), 6)
pub fn tabs(num: usize) -> String {
    format!("{:1$}", "", TAB_SIZE * num)
}

pub fn indent_strings(strs: &[String], indent: usize) -> Vec<String> {
    strs.iter().map(|s| format!("{}{}", tabs(indent), s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::defines::*;

    #[test]
    fn test_tabs() {
        assert_eq!(tabs(0), "");
        assert_eq!(tabs(1), format!("{}", TAB));
        assert_eq!(tabs(2), format!("{0}{0}", TAB));
    }

    #[test]
    fn test_indent_strings() {
        assert_eq!(indent_strings(&["a".into(), "b".into()], 1),
                   ["    a", "    b"]);
    }
}
