use std::{borrow::Cow, collections::HashMap, path::*, sync::OnceLock};

use crate::case::*;

static CRATE_NAME_OVERRIDES: OnceLock<HashMap<String, String>> = OnceLock::new();

pub(crate) fn set_crate_name_overrides(overrides: HashMap<String, String>) {
    assert!(
        CRATE_NAME_OVERRIDES.set(overrides).is_ok(),
        "Crate name overrides already set"
    );
}

fn get_crate_name_override(crate_name: &str) -> Option<String> {
    CRATE_NAME_OVERRIDES
        .get()
        .and_then(|overrides| overrides.get(crate_name).cloned())
}

pub fn split_namespace_name(name: &str) -> (Option<&str>, &str) {
    let mut parts = name.split('.');
    let name = parts.next_back().unwrap();
    let ns = parts.next_back();
    assert!(ns.is_none() || parts.next().is_none());
    (ns, name)
}

// unused :(
// pub fn strip_suffix<'a>(name: &'a str, suffix: &str) -> Option<&'a str> {
// if name.ends_with(suffix) {
// Some(&name[..name.len() - suffix.len()])
// }
// else {
// None
// }
// }

pub fn file_name_sys(name: &str) -> String {
    let mut path = PathBuf::from(name);
    let added = path.set_extension("rs");
    assert!(added);
    path.to_str().unwrap().into()
}

/// Crate name with underscores for `use` statement
pub fn crate_name(name: &str) -> String {
    let name = name.replace('-', "_").to_snake();
    let crate_name = if let Some(name_without_prefix) = name.strip_prefix("g_") {
        name_without_prefix.to_owned()
    } else {
        name
    };
    if let Some(crate_name) = get_crate_name_override(&crate_name) {
        crate_name
    } else {
        crate_name
    }
}

/// Crate name with '-' for Cargo.toml etc.
pub fn exported_crate_name(crate_name: &str) -> String {
    crate_name.replace('_', "-")
}

pub fn module_name(name: &str) -> String {
    mangle_keywords(name.to_snake()).into_owned()
}

pub fn enum_member_name(name: &str) -> String {
    if name.starts_with(char::is_alphabetic) {
        name.to_camel()
    } else {
        format!("_{}", name.to_camel())
    }
}

pub fn bitfield_member_name(name: &str) -> String {
    if name.starts_with(char::is_alphabetic) {
        name.to_uppercase()
    } else {
        format!("_{}", name.to_uppercase())
    }
}

pub fn needs_mangling(name: &str) -> bool {
    keywords().contains_key(name)
}

// If the mangling happened, guaranteed to return Owned.
pub fn mangle_keywords<'a, S: Into<Cow<'a, str>>>(name: S) -> Cow<'a, str> {
    let name = name.into();
    if let Some(s) = keywords().get(&*name) {
        s.clone().into()
    } else {
        name
    }
}

fn keywords() -> &'static HashMap<&'static str, String> {
    static KEYWORDS: OnceLock<HashMap<&'static str, String>> = OnceLock::new();
    KEYWORDS.get_or_init(|| {
        [
            "abstract", "alignof", "as", "async", "await", "become", "box", "break", "const",
            "continue", "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn",
            "for", "if", "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut",
            "offsetof", "override", "priv", "proc", "pub", "pure", "ref", "return", "Self", "self",
            "sizeof", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
            "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
        ]
        .iter()
        .map(|k| (*k, format!("{k}_")))
        .collect()
    })
}

pub fn signal_to_snake(signal: &str) -> String {
    signal.replace("::", "_").replace('-', "_")
}

pub fn lib_name_to_toml(name: &str) -> String {
    name.to_string().replace(['-', '.'], "_")
}

pub fn use_glib_type(env: &crate::env::Env, import: &str) -> String {
    format!(
        "{}::{}",
        if env.library.is_glib_crate() {
            "crate"
        } else {
            "glib"
        },
        import
    )
}

pub fn use_glib_if_needed(env: &crate::env::Env, import: &str) -> String {
    format!(
        "{}{}",
        if env.library.is_glib_crate() {
            ""
        } else {
            "glib::"
        },
        import
    )
}

pub fn use_gio_type(env: &crate::env::Env, import: &str) -> String {
    format!(
        "{}::{}",
        if env.library.is_crate("Gio") {
            "crate"
        } else {
            "gio"
        },
        import
    )
}

pub fn use_gtk_type(env: &crate::env::Env, import: &str) -> String {
    format!(
        "{}::{}",
        if env.library.is_crate("Gtk") {
            "crate"
        } else {
            "gtk"
        },
        import
    )
}

pub fn is_gstring(name: &str) -> bool {
    name == "GString" || name.ends_with("::GString")
}

#[cfg(test)]
mod tests {
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

    // #[test]
    // fn strip_prefix_g() {
    // assert_eq!(strip_prefix("G", "GBusType"), "BusType");
    // assert_eq!(strip_prefix("G", "G_BUS_TYPE_NONE"), "BUS_TYPE_NONE");
    // }
    //
    // #[test]
    // fn strip_prefix_gtk() {
    // assert_eq!(strip_prefix("Gtk", "GtkAlign"), "Align");
    // assert_eq!(strip_prefix("Gtk", "GTK_ALIGN_FILL"), "ALIGN_FILL");
    // }

    #[test]
    fn crate_name_works() {
        assert_eq!(crate_name("GdkPixbuf"), "gdk_pixbuf");
        assert_eq!(crate_name("GLib"), "glib");
        assert_eq!(crate_name("GObject"), "gobject");
        assert_eq!(crate_name("Gtk"), "gtk");
    }

    #[test]
    fn file_name_sys_works() {
        assert_eq!(file_name_sys("funcs"), "funcs.rs");
    }

    #[test]
    fn signal_to_snake_works() {
        assert_eq!(signal_to_snake("changed"), "changed");
        assert_eq!(signal_to_snake("move-active"), "move_active");
    }

    #[test]
    fn lib_name_to_toml_works() {
        assert_eq!(lib_name_to_toml("gstreamer-1.0"), "gstreamer_1_0");
    }
}
