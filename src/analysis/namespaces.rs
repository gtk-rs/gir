use std::collections::HashMap;
use std::ops::Index;

use library;

pub type NsId = u16;

#[derive(Debug)]
pub struct Namespace {
    pub name: String,
}

#[derive(Debug)]
pub struct Info {
    namespaces: Vec<Namespace>,
    name_index: HashMap<String, NsId>,
}

impl Info {
    pub fn by_name(&self, name: &str) -> Option<NsId> {
        self.name_index.get(name).cloned()
    }
}

impl Index<NsId> for Info {
    type Output = Namespace;

    fn index(&self, index: NsId) -> &Namespace {
        &self.namespaces[index as usize]
    }
}

pub fn run(gir: &library::Library) -> Info {
    let mut namespaces = Vec::new();
    let mut name_index = HashMap::new();

    for ns in gir.namespaces.iter() {
        let ns_id = namespaces.len() as NsId;
        namespaces.push(Namespace {
            name: ns.name.clone(),
        });
        name_index.insert(ns.name.clone(), ns_id);
    }

    Info {
        namespaces: namespaces,
        name_index: name_index,
    }
}
