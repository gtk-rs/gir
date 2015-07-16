use std::io::{Result, Write};

use super::super::general::write_vec;

pub fn begin<W: Write>(w: &mut W) -> Result<()>{
    let v = vec![
"",
"#![allow(non_camel_case_types)]",
"",
"extern crate libc;",
"#[macro_use] extern crate bitflags;",
    ];

    write_vec(w, &v)
}

pub fn after_extern_crates<W: Write>(w: &mut W) -> Result<()>{
    let v = vec![
"",
"#[allow(unused_imports)]",
"use libc::{c_int, c_char, c_float, c_uint, c_double,",
"    c_short, c_ushort, c_long, c_ulong,",
"    c_void, size_t, ssize_t, time_t};",
    ];

    write_vec(w, &v)
}

pub fn use_glib_ffi<W: Write>(w: &mut W) -> Result<()>{
    let v = vec![
"",
"#[allow(unused_imports)]",
"use glib_ffi::{gboolean, gconstpointer, gpointer, GType};",
    ];

    write_vec(w, &v)
}

pub fn only_for_glib<W: Write>(w: &mut W) -> Result<()>{
    let v = vec![
"",
"pub type gboolean = c_int;",
"pub const GFALSE:  c_int = 0;",
"pub const GTRUE:   c_int = 1;",
"",
"pub type gconstpointer = *const c_void;",
"pub type gpointer = *mut c_void;",
    ];

    write_vec(w, &v)
}

pub fn only_for_gtk<W: Write>(w: &mut W) -> Result<()>{
    let v = vec![
"",
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
