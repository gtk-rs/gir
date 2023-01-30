use std::{borrow::Cow, collections::HashMap, path::*};

use once_cell::sync::Lazy;

use crate::case::*;

static mut CRATE_NAME_OVERRIDES: Option<HashMap<String, String>> = None;

pub(crate) fn set_crate_name_overrides(overrides: HashMap<String, String>) {
    unsafe {
        assert!(
            CRATE_NAME_OVERRIDES.is_none(),
            "Crate name overrides already set"
        );
        CRATE_NAME_OVERRIDES = Some(overrides);
    }
}

fn get_crate_name_override(crate_name: &str) -> Option<String> {
    unsafe {
        if let Some(ref overrides) = CRATE_NAME_OVERRIDES {
            if let Some(crate_name) = overrides.get(crate_name) {
                return Some(crate_name.clone());
            }
        }
        None
    }
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

/// Crate name with undescores for `use` statement
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
    KEYWORDS.contains_key(name)
}

// If the mangling happened, guaranteed to return Owned.
pub fn mangle_keywords<'a, S: Into<Cow<'a, str>>>(name: S) -> Cow<'a, str> {
    let name = name.into();
    if let Some(s) = KEYWORDS.get(&*name) {
        s.clone().into()
    } else {
        name
    }
}

static KEYWORDS: Lazy<HashMap<&'static str, String>> = Lazy::new(|| {
    [
        "abstract", "alignof", "as", "async", "await", "become", "box", "break", "const",
        "continue", "crate", "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for",
        "if", "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "offsetof",
        "override", "priv", "proc", "pub", "pure", "ref", "return", "Self", "self", "sizeof",
        "static", "struct", "super", "trait", "true", "try", "type", "typeof", "unsafe", "unsized",
        "use", "virtual", "where", "while", "yield",
    ]
    .iter()
    .map(|k| (*k, format!("{k}_")))
    .collect()
});

pub fn signal_to_snake(signal: &str) -> String {
    signal.replace("::", "_").replace('-', "_")
}

pub fn lib_name_to_toml(name: &str) -> String {
    name.to_string().replace(['-', '.'], "_")
}

pub fn shared_lib_name_to_link_name(name: &str) -> &str {
    let mut s = name;

    if s.starts_with("lib") {
        s = &s[3..];
    }

    if let Some(offset) = s.rfind(".so") {
        s = &s[..offset];
    } else if let Some(offset) = s.rfind(".dll") {
        s = &s[..offset];
        if let Some(offset) = s.rfind('-') {
            s = &s[..offset];
        }
    }

    s
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

    #[test]
    fn shared_lib_name_to_link_name_works() {
        assert_eq!(shared_lib_name_to_link_name("libgtk-4-1.dll"), "gtk-4");
        assert_eq!(shared_lib_name_to_link_name("libatk-1.0.so.0"), "atk-1.0");
        assert_eq!(
            shared_lib_name_to_link_name("libgdk_pixbuf-2.0.so.0"),
            "gdk_pixbuf-2.0"
        );
    }
}
