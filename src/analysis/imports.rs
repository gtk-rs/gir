use std::collections::btree_map::{BTreeMap, Iter};

use version::Version;

#[derive(Clone, Debug, Default)]
pub struct Imports {
    map: BTreeMap<String, Option<Version>>,
}

impl Imports {
    pub fn new() -> Imports {
        Imports { map: BTreeMap::new() }
    }

    pub fn add(&mut self, name: &str, version: Option<Version>) {
        let entry = self.map.entry(name.to_owned()).or_insert(version);
        if version < *entry {
            *entry = version;
        }
    }

    pub fn add_used_types(&mut self, used_types: &[String], version: Option<Version>) {
        for s in used_types {
            if let Some(i) = s.find("::") {
                if i == 0 {
                    self.add(&s[2..], version);
                } else {
                    self.add(&s[..i], version);
                }
            } else {
                self.add(&s, version);
            }
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.map.remove(name);
    }

    pub fn iter(&self) -> Iter<String, Option<Version>> {
        self.map.iter()
    }
}
