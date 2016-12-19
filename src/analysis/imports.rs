use std::collections::btree_map::{BTreeMap, Iter};

use env::Env;
use super::namespaces;
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

    pub fn add_used_type(&mut self, used_type: &str, version: Option<Version>) {
        if let Some(i) = used_type.find("::") {
            if i == 0 {
                self.add(&used_type[2..], version);
            } else {
                self.add(&used_type[..i], version);
            }
        } else {
            self.add(&used_type, version);
        }
    }

    pub fn add_used_types(&mut self, used_types: &[String], version: Option<Version>) {
        for s in used_types {
            self.add_used_type(s, version);
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.map.remove(name);
    }

    pub fn clean_glib(&mut self, env: &Env) {
        if env.namespaces.glib_ns_id != namespaces::MAIN { return; }
        let glibs: Vec<(String, Option<Version>)> = self.map.iter().filter_map(|p| {
            let glib_offset = p.0.find("glib::");
            if let Some(glib_offset) = glib_offset {
                if glib_offset ==  0 {
                    Some((p.0.clone(), p.1.clone()))
                } else {
                    None
                }
            } else {
                None
            }
        }).collect();
        for p in glibs {
            self.remove(&p.0);
            self.add(&p.0[6..], p.1);
        }
    }

    pub fn iter(&self) -> Iter<String, Option<Version>> {
        self.map.iter()
    }
}
