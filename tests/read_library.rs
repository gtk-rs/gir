//! This test module contains tests that use files read_library\*.gir for checking library load

extern crate libgir as gir;

mod test_util;

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use gir::tests_export::*;
use gir::*;
use test_util::*;

fn read_library<P: AsRef<Path>>(filename: P) -> Result<(library::Library), io::Error> {
    let cfg = create_default_config();
    read_library_with_config(filename, cfg)
}

fn read_library_with_config<P: AsRef<Path>>(
    filename: P,
    cfg: Config,
) -> Result<(library::Library), io::Error> {
    let full_name = Path::new("tests")
        .join("read_library")
        .join(filename.as_ref());
    let mut library = library::Library::new(&cfg.library_name);
    let (f, _) = read_parameterized(full_name)?;
    library
        .read_reader(f, Some(default_include_dir().as_ref()))
        .unwrap();
    Ok(library)
}

#[test]
fn load_normal() {
    let cfg = create_default_config();
    let mut library = library::Library::new(&cfg.library_name);

    let f = File::open("tests/read_library/normal.gir").unwrap();
    library.read_reader(f, None).unwrap();
    assert!(library.find_type(1, "TimeSpan").is_some());
}

#[test]
fn read_mode_default() {
    let (mut f, _) = read_parameterized("tests/read_library/normal.gir").unwrap();
    let mut buffer = vec![0u8; 6];
    assert_eq!(f.read(&mut buffer).unwrap(), 6);
    let str = String::from_utf8_lossy(&buffer);
    assert_eq!(str, "<?xml ");
}

#[test]
fn read_mode_full() {
    let (mut f, _) = read_parameterized("tests/read_library/mode_full.gir").unwrap();
    let mut buffer = vec![0u8; 6];
    assert_eq!(f.read(&mut buffer).unwrap(), 6);
    let str = String::from_utf8_lossy(&buffer);
    assert_eq!(str, "<?xml ");
}

#[test]
fn read_mode_object() {
    let (mut f, _) = read_parameterized("tests/read_library/mode_object.gir").unwrap();
    let mut buffer = vec![0u8; 6];
    assert_eq!(f.read(&mut buffer).unwrap(), 6);
    let str = String::from_utf8_lossy(&buffer);
    assert_eq!(str, "<?xml ");
}

/// Check that we can add an object to glib and still have access to its objects
#[test]
fn read_glib_addition() {
    let mut cfg = create_default_config();
    cfg.library_name = "GLib".into();
    cfg.library_version = "2.0".into();
    let library = read_library_with_config("glib_addition.gir", cfg).unwrap();
    let a: &library::Alias = get_type(&library, "NewAlias");
    assert_eq!(a.c_identifier, "TNewAlias");
    assert_eq!(a.target_c_type, "gint64");
    assert_eq!(a.typ.full_name(&library), "*.Int64");
    let r: &library::Record = get_type(&library, "Variant");
    assert_eq!(r.c_type, "GVariant");
}

#[test]
fn load_alias() {
    let library = read_library("alias.gir").unwrap();
    let a: &library::Alias = get_type(&library, "TimeSpan");
    assert_eq!(a.c_identifier, "TTimeSpan");
    assert_eq!(a.target_c_type, "gint64");
    assert_eq!(a.typ.full_name(&library), "*.Int64");
}

#[test]
fn load_bitfield() {
    let library = read_library("bitfield.gir").unwrap();
    let b: &library::Bitfield = get_type(&library, "AsciiType");
    assert_eq!(b.c_type, "TAsciiType");
    let m = &b.members[0];
    assert_eq!(m.name, "alnum");
    assert_eq!(m.c_identifier, "T_ASCII_ALNUM");
    assert_eq!(m.value, "1");
    let m = &b.members[1];
    assert_eq!(m.name, "alpha");
    assert_eq!(m.c_identifier, "T_ASCII_ALPHA");
    assert_eq!(m.value, "2");
}

#[test]
fn load_class() {
    let library = read_library("class.gir").unwrap();
    let typ_app_info = library.find_type(0, "Tst.AppInfo").unwrap();
    let c: &library::Class = get_type(&library, "AppLaunchContext");
    assert_eq!(c.c_type, "GAppLaunchContext");
    assert_eq!(c.type_struct, Some("AppLaunchContextClass".into()));
    assert_eq!(c.glib_get_type, "g_app_launch_context_get_type");
    assert_eq!(c.parent.unwrap().full_name(&library), "GObject.Object");
    assert_eq!(c.version, None);
    let f = &c.functions[0];
    assert_eq!(f.name, "new");
    assert_eq!(f.c_identifier, Some("g_app_launch_context_new".into()));
    assert_eq!(f.kind, library::FunctionKind::Constructor);
    assert_eq!(f.throws, false);
    assert_eq!(f.version, None);
    let f = &c.functions[1];
    assert_eq!(f.name, "get_environment");
    assert_eq!(
        f.c_identifier,
        Some("g_app_launch_context_get_environment".into())
    );
    assert_eq!(f.kind, library::FunctionKind::Method);
    assert_eq!(f.throws, false);
    assert_eq!(f.version, Some(Version::Full(2, 32, 0)));
    let f = &c.fields[0];
    assert_eq!(f.name, "parent_instance");
    assert_eq!(f.c_type, Some("GObject".into()));
    assert_eq!(f.typ.full_name(&library), "GObject.Object");
    assert_eq!(f.private, false);
    let s = &c.signals[0];
    assert_eq!(s.name, "launched");
    assert_eq!(s.is_action, false);
    assert_eq!(s.version, Some(Version::Full(2, 36, 0)));
    let p = &s.parameters[0];
    assert_eq!(p.name, "info");
    assert_eq!(p.c_type, library::EMPTY_CTYPE);
    assert_eq!(p.typ, typ_app_info);
    assert_eq!(p.transfer, library::Transfer::None);
    let p = &s.parameters[1];
    assert_eq!(p.name, "platform_data");
    assert_eq!(p.c_type, library::EMPTY_CTYPE);
    assert_eq!(p.typ.full_name(&library), "GLib.Variant");
    assert_eq!(p.transfer, library::Transfer::None);
    let p = &s.ret;
    assert_eq!(p.name, "");
    assert_eq!(p.c_type, "void");
    assert_eq!(p.typ.full_name(&library), "*.None");
    assert_eq!(p.transfer, library::Transfer::None);
}

#[test]
fn load_constant() {
    let library = read_library("constant.gir").unwrap();
    let ns = library.namespace(library::MAIN_NAMESPACE);
    let c = &ns.constants[0];
    assert_eq!(c.name, "ANALYZER_ANALYZING");
    assert_eq!(c.c_identifier, "T_ANALYZER_ANALYZING");
    assert_eq!(c.c_type, "gint");
    assert_eq!(c.typ.full_name(&library), "*.Int");
    assert_eq!(c.value, "1");
}

#[test]
fn load_enumeration() {
    let library = read_library("enumeration.gir").unwrap();
    let e: &library::Enumeration = get_type(&library, "BookmarkFileError");
    assert_eq!(e.c_type, "TBookmarkFileError");
    assert_eq!(e.error_domain, Some("t-bookmark-file-error-quark".into()));
    let m = &e.members[0];
    assert_eq!(m.name, "invalid_uri");
    assert_eq!(m.c_identifier, "T_BOOKMARK_FILE_ERROR_INVALID_URI");
    assert_eq!(m.value, "0");
    let m = &e.members[1];
    assert_eq!(m.name, "invalid_value");
    assert_eq!(m.c_identifier, "T_BOOKMARK_FILE_ERROR_INVALID_VALUE");
    assert_eq!(m.value, "1");
}

#[test]
fn load_function() {
    let library = read_library("function.gir").unwrap();
    let ns = library.namespace(library::MAIN_NAMESPACE);
    let f = &ns.functions[0];
    assert_eq!(f.name, "access");
    assert_eq!(f.c_identifier, Some("t_access".into()));
    assert_eq!(f.kind, library::FunctionKind::Global);
    assert_eq!(f.throws, false);
    assert_eq!(f.version, Some(Version::Full(2, 8, 0)));
    let p = &f.parameters[0];
    assert_eq!(p.name, "filename");
    assert_eq!(p.c_type, "const gchar*");
    assert_eq!(p.typ.full_name(&library), "*.Filename");
    assert_eq!(p.transfer, library::Transfer::Full);
    let p = &f.parameters[1];
    assert_eq!(p.name, "mode");
    assert_eq!(p.c_type, "int");
    assert_eq!(p.typ.full_name(&library), "*.Int");
    assert_eq!(p.transfer, library::Transfer::None);
    let p = &f.ret;
    assert_eq!(p.name, "");
    assert_eq!(p.c_type, "int");
    assert_eq!(p.typ.full_name(&library), "*.Int");
    assert_eq!(p.transfer, library::Transfer::None);
}

//TODO: interface, record, union, callback
