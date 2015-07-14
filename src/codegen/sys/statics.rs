use std::io::{Result, Write};

use super::super::general::write_vec;

pub fn begin<W: Write>(w: &mut W) -> Result<()>{
    let v = vec![
"",
"#![allow(non_camel_case_types)]",
"#![allow(dead_code)]",
"",
"extern crate libc;",
"#[macro_use] extern crate bitflags;",
"",
"pub mod enums;",
    ];

    write_vec(w, &v)
}

pub fn after_extern_crates<W: Write>(w: &mut W) -> Result<()>{
    let v = vec![
"",
"#[allow(unused_imports)]",
"use libc::{c_int, c_char, c_float, c_uint, c_double, c_long, c_void, size_t, ssize_t, time_t};",
"",
"pub use glib_ffi::{",
"    gboolean, GFALSE, GTRUE, gsize, gpointer, GType, GObject, GPermission,",
"    GList, GSList, GError, GValue};",
    ];

    write_vec(w, &v)
}

pub fn before_func<W: Write>(w: &mut W) -> Result<()>{
    let v = vec![
"pub const GTK_ENTRY_BUFFER_MAX_SIZE: u16 = ::std::u16::MAX;",
"",
"//pub type GtkTreeModelForeachFunc = fn(model: *mut GtkTreeModel, path: *mut GtkTreePath, iter: *mut GtkTreeIter, data: gpointer) -> gboolean;",
"",
"pub const GTK_STYLE_PROVIDER_PRIORITY_FALLBACK: u32 = 1;",
"pub const GTK_STYLE_PROVIDER_PRIORITY_THEME: u32 = 200;",
"pub const GTK_STYLE_PROVIDER_PRIORITY_SETTINGS: u32 = 400;",
"pub const GTK_STYLE_PROVIDER_PRIORITY_APPLICATION: u32 = 600;",
"pub const GTK_STYLE_PROVIDER_PRIORITY_USER: u32 = 800;",
    ];

    write_vec(w, &v)
}
