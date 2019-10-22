use super::{
    external_libraries::{read_external_libraries, ExternalLibrary},
    gobjects, WorkMode,
};
use crate::{
    config::error::TomlHelper,
    git::repo_hash,
    library::{self, Library},
    nameutil::set_crate_name_overrides,
    version::Version,
};
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};
use toml;

#[derive(Debug)]
pub struct Config {
    pub work_mode: WorkMode,
    pub girs_dir: PathBuf,
    pub girs_version: String, //Version in girs_dir, detected by git
    pub library_name: String,
    pub library_version: String,
    pub target_path: PathBuf,
    /// Path where files generated in normal and sys mode
    pub auto_path: PathBuf,
    pub doc_target_path: PathBuf,
    pub external_libraries: Vec<ExternalLibrary>,
    pub objects: gobjects::GObjects,
    pub min_cfg_version: Version,
    pub make_backup: bool,
    pub generate_safety_asserts: bool,
    pub deprecate_by_min_version: bool,
    pub show_statistics: bool,
    pub concurrency: library::Concurrency,
    pub single_version_file: Option<PathBuf>,
    pub generate_display_trait: bool,
    pub docs_rs_features: Vec<String>,
}

impl Config {
    pub fn new<'a, S, B, W>(
        config_file: S,
        work_mode: W,
        girs_dir: S,
        library_name: S,
        library_version: S,
        target_path: S,
        doc_target_path: S,
        make_backup: B,
        show_statistics: B,
    ) -> Result<Config, String>
    where
        S: Into<Option<&'a str>>,
        B: Into<Option<bool>>,
        W: Into<Option<WorkMode>>,
    {
        let config_file: PathBuf = match config_file.into() {
            Some("") | None => "Gir.toml",
            Some(a) => a,
        }
        .into();

        let config_dir = match config_file.parent() {
            Some(path) => path.into(),
            None => PathBuf::new(),
        };

        let toml = match read_toml(&config_file) {
            Ok(toml) => toml,
            Err(e) => {
                return Err(format!(
                    "Error while reading \"{}\": {}",
                    config_file.display(),
                    e
                ))
            }
        };

        let overrides = read_crate_name_overrides(&toml);
        if !overrides.is_empty() {
            set_crate_name_overrides(overrides);
        }

        let work_mode = match work_mode.into() {
            Some(w) => w,
            None => {
                let s = match toml.lookup_str("options.work_mode", "No options.work_mode") {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(format!(
                            "Invalid toml file \"{}\": {}",
                            config_file.display(),
                            e
                        ))
                    }
                };
                WorkMode::from_str(s)?
            }
        };

        let girs_dir: PathBuf = match girs_dir.into() {
            Some("") | None => {
                let path = toml.lookup_str("options.girs_dir", "No options.girs_dir")?;
                config_dir.join(path)
            }
            Some(a) => a.into(),
        };
        let girs_version = repo_hash(&girs_dir).unwrap_or_else(|| "???".into());

        let (library_name, library_version) = match (library_name.into(), library_version.into()) {
            (Some(""), Some("")) | (None, None) => (
                toml.lookup_str("options.library", "No options.library")?
                    .to_owned(),
                toml.lookup_str("options.version", "No options.version")?
                    .to_owned(),
            ),
            (Some(""), Some(_)) | (Some(_), Some("")) | (None, Some(_)) | (Some(_), None) => {
                return Err("Library and version can not be specified separately".to_owned())
            }
            (Some(a), Some(b)) => (a.to_owned(), b.to_owned()),
        };

        let target_path: PathBuf = match target_path.into() {
            Some("") | None => {
                let path = toml.lookup_str("options.target_path", "No target path specified")?;
                config_dir.join(path)
            }
            Some(a) => a.into(),
        };

        let auto_path = match toml.lookup("options.auto_path") {
            Some(p) => target_path.join(p.as_result_str("options.auto_path")?),
            None if work_mode == WorkMode::Normal => target_path.join("src").join("auto"),
            None => target_path.join("src"),
        };

        let doc_target_path: PathBuf = match doc_target_path.into() {
            Some("") | None => match toml.lookup("options.doc_target_path") {
                Some(p) => config_dir.join(p.as_result_str("options.doc_target_path")?),
                None => target_path.join("vendor.md"),
            },
            Some(p) => config_dir.join(p),
        };

        let concurrency = match toml.lookup("options.concurrency") {
            Some(v) => v.as_result_str("options.concurrency")?.parse()?,
            None => Default::default(),
        };

        let generate_display_trait = match toml.lookup("options.generate_display_trait") {
            Some(v) => v.as_result_bool("options.generate_display_trait")?,
            None => true,
        };

        let mut docs_rs_features = Vec::new();
        for v in match toml.lookup("options.docs_rs_features") {
            Some(v) => v.as_result_vec("options.docs_rs_features")?.as_slice(),
            None => &[],
        } {
            docs_rs_features.push(match v.as_str() {
                Some(s) => s.to_owned(),
                None => {
                    return Err(format!(
                        "Invalid `docs_rs_features` value element, expected a string, found {}",
                        v.type_str()
                    ))
                }
            });
        }

        // options.concurrency is the default of all objects if nothing
        // else is configured
        let mut objects = toml
            .lookup("object")
            .map(|t| gobjects::parse_toml(t, concurrency, generate_display_trait))
            .unwrap_or_default();
        gobjects::parse_status_shorthands(&mut objects, &toml, concurrency, generate_display_trait);
        gobjects::parse_builders(&mut objects, &toml);

        let external_libraries = read_external_libraries(&toml)?;

        let min_cfg_version = match toml.lookup("options.min_cfg_version") {
            Some(v) => v.as_result_str("options.min_cfg_version")?.parse()?,
            None => Default::default(),
        };

        let generate_safety_asserts = match toml.lookup("options.generate_safety_asserts") {
            Some(v) => v.as_result_bool("options.generate_safety_asserts")?,
            None => false,
        };

        let deprecate_by_min_version = match toml.lookup("options.deprecate_by_min_version") {
            Some(v) => v.as_result_bool("options.deprecate_by_min_version")?,
            None => false,
        };

        let single_version_file = match toml.lookup("options.single_version_file") {
            Some(v) => match v.as_result_bool("options.single_version_file") {
                Ok(false) => None,
                Ok(true) => Some(make_single_version_file(None, &target_path)),
                Err(_) => match v.as_str() {
                    Some(p) => Some(make_single_version_file(Some(p), &target_path)),
                    None => return Err("single_version_file must be bool or string path".into()),
                },
            },
            None => None,
        };

        Ok(Config {
            work_mode,
            girs_dir,
            girs_version,
            library_name,
            library_version,
            target_path,
            auto_path,
            doc_target_path,
            external_libraries,
            objects,
            min_cfg_version,
            make_backup: make_backup.into().unwrap_or(false),
            generate_safety_asserts,
            deprecate_by_min_version,
            show_statistics: show_statistics.into().unwrap_or(false),
            concurrency,
            single_version_file,
            generate_display_trait,
            docs_rs_features,
        })
    }

    pub fn library_full_name(&self) -> String {
        format!("{}-{}", self.library_name, self.library_version)
    }

    pub fn filter_version(&self, version: Option<Version>) -> Option<Version> {
        version.and_then(|v| {
            if v > self.min_cfg_version {
                Some(v)
            } else {
                None
            }
        })
    }

    pub fn resolve_type_ids(&mut self, library: &Library) {
        gobjects::resolve_type_ids(&mut self.objects, library)
    }
}

fn read_toml<P: AsRef<Path>>(filename: P) -> Result<toml::Value, String> {
    if !filename.as_ref().is_file() {
        return Err("Config don't exists or not file".to_owned());
    }
    let mut input = String::new();
    match File::open(&filename) {
        Ok(mut f) => {
            if let Err(e) = f.read_to_string(&mut input) {
                return Err(format!(
                    "read_to_string failed on \"{}\": {}",
                    filename.as_ref().display(),
                    e
                ));
            }

            match toml::from_str(&input) {
                Ok(toml) => Ok(toml),
                Err(e) => Err(format!(
                    "Invalid toml format in \"{}\": {}",
                    filename.as_ref().display(),
                    e
                )),
            }
        }
        Err(e) => Err(format!(
            "Cannot open file \"{}\": {}",
            filename.as_ref().display(),
            e
        )),
    }
}

fn make_single_version_file(configured: Option<&str>, target_path: &Path) -> PathBuf {
    let file_dir = match configured {
        None | Some("") => target_path.join("src").join("auto"),
        Some(path) => target_path.join(path),
    };

    if file_dir.extension().is_some() {
        file_dir
    } else {
        file_dir.join("versions.txt")
    }
}

fn read_crate_name_overrides(toml: &toml::Value) -> HashMap<String, String> {
    let mut overrides = HashMap::new();
    if let Some(a) = toml
        .lookup("crate_name_overrides")
        .and_then(toml::Value::as_table)
    {
        for (key, value) in a {
            if let Some(s) = value.as_str() {
                overrides.insert(key.clone(), s.to_string());
            }
        }
    };
    overrides
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_single_version_file() {
        let target_path = Path::new("/tmp/glib");
        assert_eq!(
            make_single_version_file(None, &target_path),
            PathBuf::from("/tmp/glib/src/auto/versions.txt")
        );
        assert_eq!(
            make_single_version_file(Some(""), &target_path),
            PathBuf::from("/tmp/glib/src/auto/versions.txt")
        );
        assert_eq!(
            make_single_version_file(Some("src"), &target_path),
            PathBuf::from("/tmp/glib/src/versions.txt")
        );
        assert_eq!(
            make_single_version_file(Some("src/vers.txt"), &target_path),
            PathBuf::from("/tmp/glib/src/vers.txt")
        );
        assert_eq!(
            make_single_version_file(Some("."), &target_path),
            PathBuf::from("/tmp/glib/versions.txt")
        );
        assert_eq!(
            make_single_version_file(Some("./_vers.dat"), &target_path),
            PathBuf::from("/tmp/glib/_vers.dat")
        );
    }
}
