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

    pub fn add(&mut self, name: String, version: Option<Version>) {
        let entry = self.map.entry(name).or_insert(version);
        if version < *entry {
            *entry = version;
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.map.remove(name);
    }

    pub fn iter(&self) -> Iter<String, Option<Version>> {
        self.map.iter()
    }
}
