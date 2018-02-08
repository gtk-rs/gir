use std::collections::btree_map::{BTreeMap, Iter};

use library::Library;
use nameutil::crate_name;
use super::namespaces;
use version::Version;

/// Provides assistance in generating use declarations.
///
/// It takes into account that use declaration referring to names within the
/// same crate will look differently. It also avoids generating spurious
/// declarations referring to names from within the same module as the one we
/// are generating code for.
#[derive(Clone, Debug, Default)]
pub struct Imports {
    /// Name of the current crate.
    crate_name: String,
    /// Name defined within current module. It doesn't need use declaration.
    ///
    /// NOTE: Currently we don't need to support more than one such name.
    defined: Option<String>,
    map: BTreeMap<String, Option<Version>>,
}

impl Imports {
    pub fn new(gir: &Library) -> Imports {
        let crate_name = crate_name(&gir.namespace(namespaces::MAIN).name);
        Imports {
            crate_name,
            defined: None,
            map: BTreeMap::new(),
        }
    }

    pub fn with_defined(gir: &Library, name: &str) -> Imports {
        let crate_name = crate_name(&gir.namespace(namespaces::MAIN).name);
        Imports {
            crate_name,
            defined: Some(name.to_owned()),
            map: BTreeMap::new(),
        }
    }

    /// Declares that name should be available through its last path component.
    ///
    /// For example, if name is `X::Y::Z` then it will be available as `Z`.
    pub fn add(&mut self, name: &str, version: Option<Version>) {
        if let Some(ref defined) = self.defined {
            if name == defined {
                return
            }
        }
        if let Some(name) = self.strip_crate_name(name) {
            let entry = self.map.entry(name.to_owned()).or_insert(version);
            if version < *entry {
                *entry = version;
            }
        }
    }

    /// Declares that name should be available through its full path.
    ///
    /// For example, if name is `X::Y` then it will be available as `X::Y`.
    pub fn add_used_type(&mut self, used_type: &str, version: Option<Version>) {
        if let Some(i) = used_type.find("::") {
            if i == 0 {
                self.add(&used_type[2..], version);
            } else {
                self.add(&used_type[..i], version);
            }
        } else {
            self.add(used_type, version);
        }
    }

    pub fn add_used_types(&mut self, used_types: &[String], version: Option<Version>) {
        for s in used_types {
            self.add_used_type(s, version);
        }
    }

    /// Tries to strip crate name prefix from given name.
    ///
    /// Returns `None` if name matches crate name exactly. Otherwise returns
    /// name with crate name prefix stripped or full name if there was no match.
    fn strip_crate_name<'a>(&self, name: &'a str) -> Option<&'a str> {
        let prefix = &self.crate_name;
        if !name.starts_with(prefix) {
            return Some(name);
        }
        let rest = &name[prefix.len()..];
        if rest.is_empty() {
            None
        } else if rest.starts_with("::") {
            Some(&rest["::".len()..])
        } else {
            // It was false positive, return the whole name.
            Some(name)
        }
    }

    pub fn iter(&self) -> Iter<String, Option<Version>> {
        self.map.iter()
    }
}
