use super::error::*;
use crate::nameutil::crate_name;
use toml;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalLibrary {
    pub namespace: String,
    pub crate_name: String,
}

pub fn read_external_libraries(toml: &toml::Value) -> Result<Vec<ExternalLibrary>, String> {
    let mut external_libraries = match toml.lookup("options.external_libraries") {
        Some(a) => a
            .as_result_vec("options.external_libraries")?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .map(|namespace| ExternalLibrary {
                crate_name: crate_name(&namespace),
                namespace,
            })
            .collect(),
        None => Vec::new(),
    };
    let custom_libs = toml
        .lookup("external_libraries")
        .and_then(toml::Value::as_table);
    if let Some(custom_libs) = custom_libs {
        for custom_lib in custom_libs {
            let crate_name = custom_lib.0;
            if let Some(namespace) = custom_lib.1.as_str() {
                let lib = ExternalLibrary {
                    namespace: namespace.to_owned(),
                    crate_name: crate_name.clone(),
                };
                external_libraries.push(lib);
            } else {
                return Err(format!(
                    "For external library \"{}\" namespace must be string",
                    crate_name
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
        let value = ::toml::from_str(&input);
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
            }
        );
        assert_eq!(
            libs[1],
            ExternalLibrary {
                namespace: "Gdk".to_owned(),
                crate_name: "gdk".to_owned(),
            }
        );
        assert_eq!(
            libs[2],
            ExternalLibrary {
                namespace: "GdkPixbuf".to_owned(),
                crate_name: "gdk_pixbuf".to_owned(),
            }
        );
        //Sorted alphabetically
        assert_eq!(
            libs[3],
            ExternalLibrary {
                namespace: "CoolLib".to_owned(),
                crate_name: "coollib".to_owned(),
            }
        );
        assert_eq!(
            libs[4],
            ExternalLibrary {
                namespace: "OtherLib".to_owned(),
                crate_name: "other-lib".to_owned(),
            }
        );
    }
}
