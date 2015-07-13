use std::ascii::AsciiExt;
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::*;
use std::string::String;

use case::*;


pub fn split_namespace_name(name: &str) -> (Option<&str>, &str) {
    let mut parts = name.split('.');
    let name = parts.next_back().unwrap();
    let ns = parts.next_back();
    assert!(ns.is_none() || parts.next().is_none());
    (ns, name)
}

pub fn strip_prefix<'a>(prefix: &str, name: &'a str) -> &'a str {
    let mut skip = 0;
    let mut prefix_upper = prefix.to_ascii_uppercase();
    prefix_upper.push('_');
    if name.starts_with(&prefix_upper) {
        skip = prefix_upper.len();
    }
    else if name.starts_with(prefix) {
        skip = prefix.len();
    }
    &name[skip..]
}

pub fn strip_suffix<'a>(name: &'a str, suffix: &str) -> Option<&'a str> {
    if name.ends_with(suffix) {
        Some(&name[..name.len() - suffix.len()])
    }
    else {
        None
    }
}

pub fn file_name(full_name: &str) -> String {
    let (_, class_name) = split_namespace_name(full_name);
    let mut name = PathBuf::from(module_name(class_name));
    let added = name.set_extension("rs");
    assert!(added);
    name.to_str().unwrap().into()
}

pub fn file_name_sys(library_name: &str, name: &str) -> String {
    let mut path = PathBuf::from(format!("{}-sys", module_name(library_name)))
        .join("src").join(name);
    let added = path.set_extension("rs");
    assert!(added);
    path.to_str().unwrap().into()
}

pub fn module_name(name: &str) -> String {
    name.to_snake()
}

pub fn mangle_keywords<'a, S: Into<Cow<'a, str>>>(name: S) -> Cow<'a, str> {
    let name = name.into();
    if let Some(s) = KEYWORDS.get(&*name) {
        s[..].into()
    }
    else {
        name
    }
}

lazy_static! {
    static ref KEYWORDS: HashMap<&'static str, String> = {
        let mut map = HashMap::new();
        [
            "abstract", "alignof", "as", "become", "box", "break", "const",
            "continue", "crate", "do", "else", "enum", "extern", "false", "final",
            "fn", "for", "if", "impl", "in", "let", "loop", "macro", "match", "mod",
            "move", "mut", "offsetof", "override", "priv", "proc", "pub", "pure",
            "ref", "return", "Self", "self", "sizeof", "static", "struct", "super",
            "trait", "true", "type", "typeof", "unsafe", "unsized", "use", "virtual",
            "where", "while", "yield",
        ].iter().map(|k| map.insert(*k, format!("{}_", k))).count();
        map
    };
}

#[cfg(test)]
mod tests {
    use std::path::*;
    use super::*;

    #[test]
    fn split_no_namespace() {
        let (ns, name) = split_namespace_name("GObject");
        assert_eq!(ns, None);
        assert_eq!(name, "GObject");
    }

    #[test]
    fn split_full_name() {
        let (ns, name) = split_namespace_name("Gtk.StatusIcon");
        assert_eq!(ns, Some("Gtk"));
        assert_eq!(name, "StatusIcon");
    }

    #[test]
    fn strip_prefix_g() {
        assert_eq!(strip_prefix("G", "GBusType"), "BusType");
        assert_eq!(strip_prefix("G", "G_BUS_TYPE_NONE"), "BUS_TYPE_NONE");
    }

    #[test]
    fn strip_prefix_gtk() {
        assert_eq!(strip_prefix("Gtk", "GtkAlign"), "Align");
        assert_eq!(strip_prefix("Gtk", "GTK_ALIGN_FILL"), "ALIGN_FILL");
    }

    #[test]
    fn file_name_works() {
        assert_eq!(file_name("Gtk.StatusIcon"), "status_icon.rs");
    }

    #[test]
    fn file_name_sys_works() {
        let expected: String = PathBuf::from("gtk-sys").join("src").join("funcs.rs")
            .to_str().unwrap().into();
        assert_eq!(file_name_sys("Gtk", "funcs"), expected);
    }
}
