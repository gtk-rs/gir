use super::defines::*;

//TODO: convert to macro with usage
//format!(indent!(5, "format:{}"), 6)
pub fn tabs(num: usize) -> String {
    format!("{:1$}", "", TAB_SIZE * num)
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
}
