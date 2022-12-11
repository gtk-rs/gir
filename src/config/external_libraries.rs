use std::str::FromStr;

use super::error::*;
use crate::{nameutil::crate_name, version::Version};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalLibrary {
    pub namespace: String,
    pub crate_name: String,
    pub lib_name: String,
    pub min_version: Option<Version>,
}

pub fn read_external_libraries(toml: &toml::Value) -> Result<Vec<ExternalLibrary>, String> {
    let mut external_libraries = match toml.lookup("options.external_libraries") {
        Some(a) => a
            .as_result_vec("options.external_libraries")?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .map(|namespace| {
                let crate_name_ = crate_name(&namespace);
                ExternalLibrary {
                    crate_name: crate_name_.clone(),
                    lib_name: crate_name_,
                    min_version: None,
                    namespace,
                }
            })
            .collect(),
        None => Vec::new(),
    };
    let custom_libs = toml
        .lookup("external_libraries")
        .and_then(toml::Value::as_table);
    if let Some(custom_libs) = custom_libs {
        for custom_lib in custom_libs {
            if let Some(info) = custom_lib.1.as_table() {
                let namespace = custom_lib.0.as_str();
                let crate_name_ = info.get("crate").map_or_else(
                    || crate_name(namespace),
                    |c| c.as_str().expect("crate name must be a string").to_string(),
                );
                let min_version = info
                    .get("min_version")
                    .map(|v| v.as_str().expect("min required version must be a string"))
                    .map(|v| Version::from_str(v).expect("Invalid version number"));
                let lib = ExternalLibrary {
                    namespace: namespace.to_owned(),
                    crate_name: crate_name_,
                    lib_name: crate_name(namespace),
                    min_version,
                };
                external_libraries.push(lib);
            } else if let Some(namespace) = custom_lib.1.as_str() {
                let crate_name_ = custom_lib.0;
                let lib = ExternalLibrary {
                    namespace: namespace.to_owned(),
                    crate_name: crate_name_.clone(),
                    lib_name: crate_name(custom_lib.1.as_str().expect("No custom lib name set")),
                    min_version: None,
                };
                external_libraries.push(lib);
            } else {
                return Err(format!(
                    "For external library \"{:#?}\" namespace must be string or a table",
                    custom_lib.0
                ));
            }
        }
    }

    Ok(external_libraries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toml(input: &str) -> ::toml::Value {
        let value = ::toml::from_str(input);
        assert!(value.is_ok());
        value.unwrap()
    }

    #[test]
    fn test_read_external_libraries() {
        let toml = toml(
            r#"
[options]
external_libraries = [
   "GLib",
   "Gdk",
   "GdkPixbuf",
]

[external_libraries]
coollib="CoolLib"
other-lib="OtherLib"
"#,
        );
        let libs = read_external_libraries(&toml).unwrap();

        assert_eq!(
            libs[0],
            ExternalLibrary {
                namespace: "GLib".to_owned(),
                crate_name: "glib".to_owned(),
                lib_name: "glib".to_owned(),
                min_version: None,
            }
        );
        assert_eq!(
            libs[1],
            ExternalLibrary {
                namespace: "Gdk".to_owned(),
                crate_name: "gdk".to_owned(),
                lib_name: "gdk".to_owned(),
                min_version: None,
            }
        );
        assert_eq!(
            libs[2],
            ExternalLibrary {
                namespace: "GdkPixbuf".to_owned(),
                crate_name: "gdk_pixbuf".to_owned(),
                lib_name: "gdk_pixbuf".to_owned(),
                min_version: None,
            }
        );
        // Sorted alphabetically
        assert_eq!(
            libs[3],
            ExternalLibrary {
                namespace: "CoolLib".to_owned(),
                crate_name: "coollib".to_owned(),
                lib_name: "cool_lib".to_owned(),
                min_version: None,
            }
        );
        assert_eq!(
            libs[4],
            ExternalLibrary {
                namespace: "OtherLib".to_owned(),
                crate_name: "other-lib".to_owned(),
                lib_name: "other_lib".to_owned(),
                min_version: None,
            }
        );
    }

    #[test]
    fn test_read_external_libraries_with_min_version() {
        let toml = toml(
            r#"
[external_libraries]
CoolLib={crate = "coollib", min_version = "0.3.0"}
OtherLib={min_version = "0.4.0"}
"#,
        );
        let libs = read_external_libraries(&toml).unwrap();

        // Sorted alphabetically
        assert_eq!(
            libs[0],
            ExternalLibrary {
                namespace: "CoolLib".to_owned(),
                crate_name: "coollib".to_owned(),
                lib_name: "cool_lib".to_owned(),
                min_version: Some(Version::from_str("0.3.0").unwrap()),
            }
        );
        assert_eq!(
            libs[1],
            ExternalLibrary {
                namespace: "OtherLib".to_owned(),
                crate_name: "other_lib".to_owned(),
                lib_name: "other_lib".to_owned(),
                min_version: Some(Version::from_str("0.4.0").unwrap()),
            }
        );
    }
}
