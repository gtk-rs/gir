use super::namespaces;
use crate::{library::Library, nameutil::crate_name, version::Version};
use std::collections::btree_map::{BTreeMap, Iter};

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
    defaults: ImportConditions,
    map: BTreeMap<String, ImportConditions>,
}

impl Imports {
    pub fn new(gir: &Library) -> Imports {
        Imports {
            crate_name: make_crate_name(gir),
            defined: None,
            defaults: ImportConditions::default(),
            map: BTreeMap::new(),
        }
    }

    pub fn with_defined(gir: &Library, name: &str) -> Imports {
        Imports {
            crate_name: make_crate_name(gir),
            defined: Some(name.to_owned()),
            defaults: ImportConditions::default(),
            map: BTreeMap::new(),
        }
    }

    pub fn set_defaults(&mut self, version: Option<Version>, constraint: &Option<String>) {
        let constraints = if let Some(constraint) = constraint {
            vec![constraint.clone()]
        } else {
            vec![]
        };
        self.defaults = ImportConditions {
            version,
            constraints,
        };
    }

    pub fn reset_defaults(&mut self) {
        self.defaults.clear();
    }

    /// Declares that name should be available through its last path component.
    ///
    /// For example, if name is `X::Y::Z` then it will be available as `Z`.
    /// Uses defaults
    pub fn add(&mut self, name: &str) {
        if let Some(ref defined) = self.defined {
            if name == defined {
                return;
            }
        }
        if let Some(name) = self.strip_crate_name(name) {
            let defaults = &self.defaults;
            let entry = self
                .map
                .entry(name.to_owned())
                .or_insert_with(|| defaults.clone());
            entry.update_version(self.defaults.version);
            entry.update_constraints(self.defaults.constraints.clone());
        }
    }

    /// Declares that name should be available through its last path component.
    ///
    /// For example, if name is `X::Y::Z` then it will be available as `Z`.
    pub fn add_with_version(&mut self, name: &str, version: Option<Version>) {
        if let Some(ref defined) = self.defined {
            if name == defined {
                return;
            }
        }
        if let Some(name) = self.strip_crate_name(name) {
            let entry = self.map.entry(name.to_owned()).or_insert(ImportConditions {
                version,
                constraints: Vec::new(),
            });
            entry.update_version(version);
            // Since there is no constraint on this import, if any constraint
            // is present, we can just remove it.
            entry.constraints.clear();
        }
    }

    /// Declares that name should be available through its last path component and provides
    /// an optional feature constraint.
    ///
    /// For example, if name is `X::Y::Z` then it will be available as `Z`.
    pub fn add_with_constraint(
        &mut self,
        name: &str,
        version: Option<Version>,
        constraint: Option<&str>,
    ) {
        if let Some(ref defined) = self.defined {
            if name == defined {
                return;
            }
        }
        if let Some(name) = self.strip_crate_name(name) {
            let entry = if let Some(constraint) = constraint {
                let constraint = String::from(constraint);
                let entry = self.map.entry(name.to_owned()).or_insert(ImportConditions {
                    version,
                    constraints: vec![constraint.clone()],
                });
                entry.add_constraint(constraint);
                entry
            } else {
                let entry = self.map.entry(name.to_owned()).or_insert(ImportConditions {
                    version,
                    constraints: Vec::new(),
                });
                // Since there is no constraint on this import, if any constraint
                // is present, we can just remove it.
                entry.constraints.clear();
                entry
            };
            entry.update_version(version);
        }
    }

    /// Declares that name should be available through its full path.
    ///
    /// For example, if name is `X::Y` then it will be available as `X::Y`.
    pub fn add_used_type(&mut self, used_type: &str) {
        if let Some(i) = used_type.find("::") {
            if i == 0 {
                self.add(&used_type[2..]);
            } else {
                self.add(&used_type[..i]);
            }
        } else {
            self.add(used_type);
        }
    }

    pub fn add_used_types(&mut self, used_types: &[String]) {
        for s in used_types {
            self.add_used_type(s);
        }
    }

    /// Declares that name should be available through its full path.
    ///
    /// For example, if name is `X::Y` then it will be available as `X::Y`.
    pub fn add_used_type_with_version(&mut self, used_type: &str, version: Option<Version>) {
        if let Some(i) = used_type.find("::") {
            if i == 0 {
                self.add_with_version(&used_type[2..], version);
            } else {
                self.add_with_version(&used_type[..i], version);
            }
        } else {
            self.add_with_version(used_type, version);
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

    pub fn iter(&self) -> Iter<'_, String, ImportConditions> {
        self.map.iter()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ImportConditions {
    pub version: Option<Version>,
    pub constraints: Vec<String>,
}

impl ImportConditions {
    fn clear(&mut self) {
        *self = ImportConditions::default();
    }

    fn update_version(&mut self, version: Option<Version>) {
        if version < self.version {
            self.version = version;
        }
    }

    fn add_constraint(&mut self, constraint: String) {
        // If the import is already present but doesn't have any constraint,
        // we don't want to add one.
        if self.constraints.is_empty() {
            return;
        }
        // Otherwise, we just check if the constraint
        // is already present or not before adding it.
        if !self.constraints.iter().any(|x| x == &constraint) {
            self.constraints.push(constraint);
        }
    }

    fn update_constraints(&mut self, constraints: Vec<String>) {
        // If the import is already present but doesn't have any constraint,
        // we don't want to add one.
        if self.constraints.is_empty() {
            return;
        }
        if constraints.is_empty() {
            // Since there is no constraint on this import, if any constraint
            // is present, we can just remove it.
            self.constraints.clear();
        } else {
            // Otherwise, we just check if the constraint
            // is already present or not before adding it.
            for constraint in constraints {
                if !self.constraints.iter().any(|x| x == &constraint) {
                    self.constraints.push(constraint.clone());
                }
            }
        }
    }
}

fn make_crate_name(gir: &Library) -> String {
    let name = gir.namespace(namespaces::MAIN).name.as_str();
    if name == "GObject" {
        crate_name("GLib")
    } else {
        crate_name(name)
    }
}
