use std::path::*;
use std::string::String;

use case::*;

pub fn class_name(full_name: &str) -> String {
    let mut parts = full_name.split('.');
    let name = parts.next_back().unwrap();
    name.into()
}

pub fn file_name(full_name: &str) -> String {
    let class_name = class_name(full_name);
    let mut name = PathBuf::from(&class_name.to_snake());
    let added = name.set_extension("rs");
    assert!(added);
    name.to_str().unwrap().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_name_works() {
        assert_eq!(class_name("Gtk.StatusIcon"), "StatusIcon");
    }

    #[test]
    fn file_name_works() {
        assert_eq!(file_name("Gtk.StatusIcon"), "status_icon.rs");
    }
}
