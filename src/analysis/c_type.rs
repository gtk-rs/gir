pub fn rustify_pointers(c_type: &str) -> (String, String) {
    let mut input = c_type.trim();
    let leading_const = input.starts_with("const ");
    if leading_const {
        input = &input[6..];
    }
    let end = [
        input.find(" const"),
        input.find("*const"),
        input.find("*"),
        Some(input.len()),
    ].iter().filter_map(|&x| x).min().unwrap();
    let inner = input[..end].trim().into();

    let mut ptrs: Vec<_> = input[end..].rsplit('*').skip(1)
        .map(|s| if s.contains("const") { "*const" } else { "*mut" }).collect();
    if let (true, Some(p)) = (leading_const, ptrs.last_mut()) {
        *p = "*const";
    }

    let res = (ptrs.join(" "), inner);
    trace!("rustify `{}` -> `{}` `{}`", c_type, res.0, res.1);
    res
}

#[cfg(test)]
mod tests {
    use super::rustify_pointers as rustify_ptr;

    fn s(x: &str, y: &str) -> (String, String) {
        (x.into(), y.into())
    }

    #[test]
    fn rustify_pointers() {
        assert_eq!(rustify_ptr("char"), s("", "char"));
        assert_eq!(rustify_ptr("char*"), s("*mut", "char"));
        assert_eq!(rustify_ptr("const char*"), s("*const", "char"));
        assert_eq!(rustify_ptr("char const*"), s("*const", "char"));
        assert_eq!(rustify_ptr("char const *"), s("*const", "char"));
        assert_eq!(rustify_ptr(" char * * "), s("*mut *mut", "char"));
        assert_eq!(rustify_ptr("const char**"), s("*mut *const", "char"));
        assert_eq!(rustify_ptr("char const**"), s("*mut *const", "char"));
        assert_eq!(rustify_ptr("const char* const*"), s("*const *const", "char"));
        assert_eq!(rustify_ptr("char const * const *"), s("*const *const", "char"));
        assert_eq!(rustify_ptr("char* const*"), s("*const *mut", "char"));

        assert_eq!(rustify_ptr("GtkWidget*"), s("*mut", "GtkWidget"));
    }
}
