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
"extern crate glib_sys as glib_ffi;",
"extern crate gdk_sys as gdk_ffi;",
"extern crate pango_sys as pango_ffi;",
"",
"pub mod enums;",
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
"",
"#[repr(C)]",
"pub struct GtkPaperSize; //boxed",
"#[repr(C)]",
"pub struct GtkTextIter; //boxed",
"#[repr(C)]",
"pub struct GtkTreePath; //boxed",
"#[repr(C)]",
"pub struct GtkTextAttributes;",
"#[repr(C)]",
"pub struct GtkRecentInfo;",
"#[repr(C)]",
"pub struct GtkRecentData {",
"    pub display_name: *mut c_char,",
"    pub description: *mut c_char,",
"    pub mime_type: *mut c_char,",
"    pub app_name: *mut c_char,",
"    pub app_exec: *mut c_char,",
"    pub groups: *mut *mut c_char,",
"    pub is_private: gboolean",
"}",
"#[repr(C)]",
"pub struct GtkRecentFilterInfo {",
"    pub contains: enums::RecentFilterFlags,",
"    pub uri: *const c_char,",
"    pub display_name: *const c_char,",
"    pub mime_type: *const c_char,",
"    pub applications: *const *const c_char,",
"    pub groups: *const *const c_char,",
"    pub age: c_int",
"}",
"#[repr(C)]",
"pub struct GtkTreeIter {",
"    pub stamp: c_int,",
"    pub user_data: *mut c_void,",
"    pub user_data2: *mut c_void,",
"    pub user_data3: *mut c_void",
"}",
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
