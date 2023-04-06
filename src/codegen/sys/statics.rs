use std::io::{Result, Write};

use super::super::general::write_vec;

pub fn begin(w: &mut dyn Write) -> Result<()> {
    let v = vec![
        "",
        "#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]",
        "#![allow(clippy::approx_constant, clippy::type_complexity, clippy::unreadable_literal, clippy::upper_case_acronyms)]",
        "#![cfg_attr(docsrs, feature(doc_cfg))]",
        "",
    ];

    write_vec(w, &v)
}

pub fn after_extern_crates(w: &mut dyn Write) -> Result<()> {
    let v = vec![
        "",
        "#[allow(unused_imports)]",
        "use libc::{c_int, c_char, c_uchar, c_float, c_uint, c_double,",
        "    c_short, c_ushort, c_long, c_ulong,",
        "    c_void, size_t, ssize_t, intptr_t, uintptr_t, FILE};",
    ];

    write_vec(w, &v)
}

pub fn use_glib(w: &mut dyn Write) -> Result<()> {
    let v = vec![
        "",
        "#[allow(unused_imports)]",
        "use glib::{gboolean, gconstpointer, gpointer, GType};",
    ];

    write_vec(w, &v)
}

pub fn only_for_glib(w: &mut dyn Write) -> Result<()> {
    let v = vec![
        "",
        "pub type gboolean = c_int;",
        "pub const GFALSE:  c_int = 0;",
        "pub const GTRUE:   c_int = 1;",
        "",
        "pub type gconstpointer = *const c_void;",
        "pub type gpointer = *mut c_void;",
        "",
    ];

    write_vec(w, &v)
}

pub fn only_for_gobject(w: &mut dyn Write) -> Result<()> {
    let v = vec![
        "",
        "pub const G_TYPE_INVALID: GType = 0 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_NONE: GType = 1 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_INTERFACE: GType = 2 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_CHAR: GType = 3 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_UCHAR: GType = 4 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_BOOLEAN: GType = 5 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_INT: GType = 6 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_UINT: GType = 7 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_LONG: GType = 8 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_ULONG: GType = 9 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_INT64: GType = 10 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_UINT64: GType = 11 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_ENUM: GType = 12 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_FLAGS: GType = 13 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_FLOAT: GType = 14 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_DOUBLE: GType = 15 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_STRING: GType = 16 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_POINTER: GType = 17 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_BOXED: GType = 18 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_PARAM: GType = 19 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_OBJECT: GType = 20 << G_TYPE_FUNDAMENTAL_SHIFT;",
        "pub const G_TYPE_VARIANT: GType = 21 << G_TYPE_FUNDAMENTAL_SHIFT;",
    ];

    write_vec(w, &v)
}

pub fn only_for_gtk(w: &mut dyn Write) -> Result<()> {
    let v = vec![
        "",
        "pub const GTK_ENTRY_BUFFER_MAX_SIZE: u16 = ::std::u16::MAX;",
    ];

    write_vec(w, &v)
}
