use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::{btree_map::BTreeMap, HashSet},
    ops::{Deref, DerefMut},
    vec::IntoIter,
};

use super::namespaces;
use crate::{library::Library, nameutil::crate_name, version::Version};

fn is_first_char_up(s: &str) -> bool {
    s.chars().next().unwrap().is_uppercase()
}

fn check_up_eq(a: &str, b: &str) -> Ordering {
    let is_a_up = is_first_char_up(a);
    let is_b_up = is_first_char_up(b);
    if is_a_up != is_b_up {
        if is_a_up {
            return Ordering::Greater;
        }
        return Ordering::Less;
    }
    Ordering::Equal
}

/// This function is used by the `Imports` type to generate output like `cargo
/// fmt` would.
///
/// For example:
///
/// ```text
/// use gdk; // lowercases come first.
/// use Window;
///
/// use gdk::foo; // lowercases come first here as well.
/// use gdk::Foo;
/// ```
fn compare_imports(a: &(&String, &ImportConditions), b: &(&String, &ImportConditions)) -> Ordering {
    let s = check_up_eq(a.0, b.0);
    if s != Ordering::Equal {
        return s;
    }
    let mut a = a.0.split("::");
    let mut b = b.0.split("::");
    loop {
        match (a.next(), b.next()) {
            (Some(a), Some(b)) => {
                let s = check_up_eq(a, b);
                if s != Ordering::Equal {
                    break s;
                }
                let s = a.partial_cmp(b).unwrap();
                if s != Ordering::Equal {
                    break s;
                }
            }
            (Some(_), None) => break Ordering::Greater,
            (None, Some(_)) => break Ordering::Less,
            (None, None) => break Ordering::Equal,
        }
    }
}

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
    /// Names defined within current module. It doesn't need use declaration.
    defined: HashSet<String>,
    defaults: ImportConditions,
    map: BTreeMap<String, ImportConditions>,
}

impl Imports {
    pub fn new(gir: &Library) -> Self {
        Self {
            crate_name: make_crate_name(gir),
            defined: HashSet::new(),
            defaults: ImportConditions::default(),
            map: BTreeMap::new(),
        }
    }

    pub fn with_defined(gir: &Library, name: &str) -> Self {
        Self {
            crate_name: make_crate_name(gir),
            defined: std::iter::once(name.to_owned()).collect(),
            defaults: ImportConditions::default(),
            map: BTreeMap::new(),
        }
    }

    #[must_use = "ImportsWithDefault must live while defaults are needed"]
    pub fn with_defaults(
        &mut self,
        version: Option<Version>,
        constraint: &Option<String>,
    ) -> ImportsWithDefault<'_> {
        let constraints = if let Some(constraint) = constraint {
            vec![constraint.clone()]
        } else {
            vec![]
        };
        self.defaults = ImportConditions {
            version,
            constraints,
        };

        ImportsWithDefault::new(self)
    }

    fn reset_defaults(&mut self) {
        self.defaults.clear();
    }

    /// The goals of this function is to discard unwanted imports like "crate".
    /// It also extends the checks in case you are implementing "X". For
    /// example, you don't want to import "X" or "crate::X" in this case.
    fn common_checks(&self, name: &str) -> bool {
        // The ffi namespace is used directly, including it is a programmer error.
        assert_ne!(name, "crate::ffi");

        if (!name.contains("::") && name != "xlib") || self.defined.contains(name) {
            false
        } else if let Some(name) = name.strip_prefix("crate::") {
            !self.defined.contains(name)
        } else {
            true
        }
    }

    /// Declares that `name` is defined in scope
    ///
    /// Removes existing imports from `self.map` and marks `name` as
    /// available to counter future import "requests".
    pub fn add_defined(&mut self, name: &str) {
        if self.defined.insert(name.to_owned()) {
            self.map.remove(name);
        }
    }

    /// Declares that name should be available through its last path component.
    ///
    /// For example, if name is `X::Y::Z` then it will be available as `Z`.
    /// Uses defaults.
    pub fn add(&mut self, name: &str) {
        if !self.common_checks(name) {
            return;
        }
        if let Some(mut name) = self.strip_crate_name(name) {
            if name == "xlib" {
                name = if self.crate_name == "gdk_x11" {
                    // Dirty little hack to allow to have correct import for GDKX11.
                    Cow::Borrowed("x11::xlib")
                } else {
                    // gtk has a module named "xlib" which is why this hack is needed too.
                    Cow::Borrowed("crate::xlib")
                };
            }
            let defaults = &self.defaults;
            let entry = self
                .map
                .entry(name.into_owned())
                .or_insert_with(|| defaults.clone());
            entry.update_version(self.defaults.version);
            entry.update_constraints(&self.defaults.constraints);
        }
    }

    /// Declares that name should be available through its last path component.
    ///
    /// For example, if name is `X::Y::Z` then it will be available as `Z`.
    pub fn add_with_version(&mut self, name: &str, version: Option<Version>) {
        if !self.common_checks(name) {
            return;
        }
        if let Some(name) = self.strip_crate_name(name) {
            let entry = self
                .map
                .entry(name.into_owned())
                .or_insert(ImportConditions {
                    version,
                    constraints: Vec::new(),
                });
            entry.update_version(version);
            // Since there is no constraint on this import, if any constraint
            // is present, we can just remove it.
            entry.constraints.clear();
        }
    }

    /// Declares that name should be available through its last path component
    /// and provides an optional feature constraint.
    ///
    /// For example, if name is `X::Y::Z` then it will be available as `Z`.
    pub fn add_with_constraint(
        &mut self,
        name: &str,
        version: Option<Version>,
        constraint: Option<&str>,
    ) {
        if !self.common_checks(name) {
            return;
        }
        if let Some(name) = self.strip_crate_name(name) {
            let entry = if let Some(constraint) = constraint {
                let constraint = String::from(constraint);
                let entry = self
                    .map
                    .entry(name.into_owned())
                    .or_insert(ImportConditions {
                        version,
                        constraints: vec![constraint.clone()],
                    });
                entry.add_constraint(constraint);
                entry
            } else {
                let entry = self
                    .map
                    .entry(name.into_owned())
                    .or_insert(ImportConditions {
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
            self.add(&format!("crate::{used_type}"));
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
            self.add_with_version(&format!("crate::{used_type}"), version);
        }
    }

    /// Tries to strip crate name prefix from given name.
    ///
    /// Returns `None` if name matches crate name exactly. Otherwise returns
    /// name with crate name prefix stripped or full name if there was no match.
    fn strip_crate_name<'a>(&self, name: &'a str) -> Option<Cow<'a, str>> {
        let prefix = &self.crate_name;
        if !name.starts_with(prefix) {
            return Some(Cow::Borrowed(name));
        }
        let rest = &name[prefix.len()..];
        if rest.is_empty() {
            None
        } else if rest.starts_with("::") {
            Some(Cow::Owned(format!("crate{rest}")))
        } else {
            // It was false positive, return the whole name.
            Some(Cow::Borrowed(name))
        }
    }

    pub fn iter(&self) -> IntoIter<(&String, &ImportConditions)> {
        let mut imports = self.map.iter().collect::<Vec<_>>();
        imports.sort_by(compare_imports);
        imports.into_iter()
    }
}

pub struct ImportsWithDefault<'a> {
    imports: &'a mut Imports,
}

impl<'a> ImportsWithDefault<'a> {
    fn new(imports: &'a mut Imports) -> Self {
        Self { imports }
    }
}

impl Drop for ImportsWithDefault<'_> {
    fn drop(&mut self) {
        self.imports.reset_defaults();
    }
}

impl Deref for ImportsWithDefault<'_> {
    type Target = Imports;
    fn deref(&self) -> &Self::Target {
        self.imports
    }
}

impl DerefMut for ImportsWithDefault<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.imports
    }
}

#[derive(Clone, Debug, Default, Ord, PartialEq, PartialOrd, Eq)]
pub struct ImportConditions {
    pub version: Option<Version>,
    pub constraints: Vec<String>,
}

impl ImportConditions {
    fn clear(&mut self) {
        self.version = None;
        self.constraints.clear();
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

    fn update_constraints(&mut self, constraints: &[String]) {
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
                if !self.constraints.iter().any(|x| x == constraint) {
                    self.constraints.push(constraint.clone());
                }
            }
        }
    }
}

fn make_crate_name(gir: &Library) -> String {
    if gir.is_glib_crate() {
        crate_name("GLib")
    } else {
        crate_name(gir.namespace(namespaces::MAIN).name.as_str())
    }
}
