use std::{
    collections::{HashMap, HashSet},
    iter,
};

use crate::library::*;

#[derive(Debug)]
struct Node {
    supers: Vec<TypeId>,
    subs: HashSet<TypeId>,
}

#[derive(Debug)]
pub struct Info {
    hier: HashMap<TypeId, Node>,
}

pub fn run(library: &Library) -> Info {
    let mut hier = HashMap::new();
    for (tid, _) in library.types() {
        get_node(library, &mut hier, tid);
    }
    Info { hier }
}

fn get_node<'a>(
    library: &Library,
    hier: &'a mut HashMap<TypeId, Node>,
    tid: TypeId,
) -> Option<&'a mut Node> {
    if hier.contains_key(&tid) {
        return hier.get_mut(&tid);
    }

    let direct_supers: Vec<TypeId> = match library.type_(tid) {
        Type::Class(Class {
            parent, implements, ..
        }) => parent.iter().chain(implements.iter()).copied().collect(),
        Type::Interface(Interface { prerequisites, .. }) => prerequisites.clone(),
        _ => return None,
    };

    let mut supers = Vec::new();
    for super_ in direct_supers {
        let node = get_node(library, hier, super_).expect("parent must be a class or interface");
        node.subs.insert(tid);
        for &tid in [super_].iter().chain(node.supers.iter()) {
            if !supers.contains(&tid) {
                supers.push(tid);
            }
        }
    }

    hier.insert(
        tid,
        Node {
            supers,
            subs: HashSet::new(),
        },
    );
    hier.get_mut(&tid)
}

impl Info {
    pub fn subtypes<'a>(&'a self, tid: TypeId) -> Box<dyn Iterator<Item = TypeId> + 'a> {
        match self.hier.get(&tid) {
            Some(node) => Box::new(node.subs.iter().copied()),
            None => Box::new(iter::empty()),
        }
    }

    pub fn supertypes(&self, tid: TypeId) -> &[TypeId] {
        match self.hier.get(&tid) {
            Some(node) => &node.supers,
            None => &[],
        }
    }
}
