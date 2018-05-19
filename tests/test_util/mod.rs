//! This submodule module is used in intergation tests and contains helpers

extern crate libgir as gir;

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use gir::tests_export::*;
use gir::*;

pub fn create_default_config() -> Config {
    let objects: GObjects = Default::default();
    Config {
        work_mode: WorkMode::Normal,
        girs_dir: "tmp1".into(),
        girs_version: "test".into(),
        library_name: "Tst".into(),
        library_version: "1.0".into(),
        target_path: "tmp2".into(),
        auto_path: "tmp2".into(),
        doc_target_path: "tmp2".into(),
        external_libraries: Default::default(),
        objects,
        min_cfg_version: Version::from_str("0").unwrap(),
        make_backup: false,
        generate_safety_asserts: true,
        deprecate_by_min_version: true,
        show_statistics: false,
        concurrency: library::Concurrency::None,
        single_version_file: None,
    }
}

pub fn default_include_dir() -> PathBuf {
    Path::new("tests").join("include")
}

pub type Parameters = HashMap<String, String>;

/// Read .gir file.
/// If file begins with comments, these comments are converted
/// to parameters in format name:value
/// Internally support "mode" parameter with next values:
/// "full" - file readed without changes,
/// "object" - file content is framed with repository/namespace
pub fn read_parameterized<P: AsRef<Path>>(
    filename: P,
) -> Result<(Box<Read>, Parameters), io::Error> {
    const COMMENT_START: &str = "<!--";
    const COMMENT_END: &str = "-->";

    let filename = filename.as_ref();
    let f = File::open(filename)?;
    let mut f = BufReader::new(f);

    let mut params = Parameters::new();
    params.insert("mode".into(), "full".into());

    let mut line = String::new();
    loop {
        line.clear();
        let readed = f.read_line(&mut line)?;
        if readed == 0 {
            break;
        }
        if !line.starts_with(COMMENT_START) {
            f.seek(SeekFrom::Current(-(readed as i64)))?;
            break;
        }
        let mut split = line.split(':');
        let name = split.next().unwrap();
        let name = name.trim_left_matches(COMMENT_START);
        let value = split.next().expect("No name:value in comment");
        let value = value.trim_right().trim_right_matches(COMMENT_END);
        assert!(split.next().is_none(), "Multiple : in comment");
        params.insert(name.into(), value.into());
    }

    let mode = params.get("mode").unwrap().to_owned();
    match mode.as_str() {
        "full" => Ok((Box::new(f), params)),
        "object" => {
            let begin = r##"<?xml version="1.0"?>
<repository xmlns="http://www.gtk.org/introspection/core/1.0" xmlns:c="http://www.gtk.org/introspection/c/1.0" xmlns:glib="http://www.gtk.org/introspection/glib/1.0" version="1.2">
  <package name="tst-1.0"/>
  <namespace name="Tst" version="1.0" c:identifier-prefixes="T" c:symbol-prefixes="t,tst">
"##
                .as_bytes();
            let end = r##"  </namespace>
</repository>"##
                .as_bytes();
            Ok((Box::new(begin.chain(f).chain(end)), params))
        }
        _ => panic!("unsupported mode {}", mode),
    }
}

/// Get type in main namespace
pub fn get_type<'a, T>(library: &'a library::Library, type_name: &str) -> &'a T
where
    library::Type: MaybeRef<T>,
{
    library
        .find_type(1, type_name)
        .map(|t| library.type_(t))
        .map(|t| t.to_ref_as::<T>())
        .expect(&format!("type {} not found", type_name))
}
