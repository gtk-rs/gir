use crate::{
    analysis::namespaces::{self, NsId},
    case::CaseExt,
    library::*,
};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Symbol {
    crate_name: Option<String>,
    owner_name: Option<String>,
    name: String,
}

impl Symbol {
    pub fn full_rust_name(&self) -> String {
        let mut ret = String::new();
        if let Some(ref s) = self.crate_name {
            ret.push_str(s);
            ret.push_str("::");
        }
        if let Some(ref s) = self.owner_name {
            ret.push_str(s);
            ret.push_str("::");
        }
        ret.push_str(&self.name);
        ret
    }

    pub fn make_trait_method(&mut self, trait_name: &str) {
        self.owner_name = Some(trait_name.into());
    }

    pub fn crate_name(&self) -> Option<&String> {
        self.crate_name.as_ref()
    }
}

#[derive(Debug)]
pub struct Info {
    symbols: Vec<Symbol>,
    c_name_index: HashMap<String, u32>,
    tid_index: HashMap<Option<TypeId>, u32>,
}

pub fn run(library: &Library, namespaces: &namespaces::Info) -> Info {
    let mut info = Info {
        symbols: Vec::new(),
        c_name_index: HashMap::new(),
        tid_index: HashMap::new(),
    };

    info.insert(
        "NULL",
        Symbol {
            name: "None".into(),
            ..Default::default()
        },
        None,
    );
    info.insert(
        "FALSE",
        Symbol {
            name: "false".into(),
            ..Default::default()
        },
        None,
    );
    info.insert(
        "TRUE",
        Symbol {
            name: "true".into(),
            ..Default::default()
        },
        None,
    );

    for (ns_id, ns) in library.namespaces.iter().enumerate() {
        let ns_id = ns_id as NsId;
        if ns_id == namespaces::INTERNAL {
            continue;
        }

        let crate_name = if ns_id == namespaces::MAIN {
            None
        } else {
            Some(&namespaces[ns_id].crate_name)
        };

        for (pos, typ) in ns.types.iter().map(|t| t.as_ref().unwrap()).enumerate() {
            let symbol = Symbol {
                crate_name: crate_name.cloned(),
                name: typ.get_name(),
                ..Default::default()
            };
            let tid = TypeId {
                ns_id,
                id: pos as u32,
            };

            match *typ {
                Type::Alias(Alias {
                    ref c_identifier, ..
                }) => {
                    info.insert(c_identifier, symbol, Some(tid));
                }
                Type::Enumeration(Enumeration {
                    ref name,
                    ref c_type,
                    ref members,
                    ref functions,
                    ..
                })
                | Type::Bitfield(Bitfield {
                    ref name,
                    ref c_type,
                    ref members,
                    ref functions,
                    ..
                }) => {
                    info.insert(c_type, symbol, Some(tid));
                    for member in members {
                        let symbol = Symbol {
                            crate_name: crate_name.cloned(),
                            owner_name: Some(name.clone()),
                            name: member.name.to_camel(),
                        };
                        info.insert(&member.c_identifier, symbol, Some(tid));
                    }
                    for func in functions {
                        let symbol = Symbol {
                            crate_name: crate_name.cloned(),
                            owner_name: Some(name.clone()),
                            name: func.name.clone(),
                        };
                        info.insert(func.c_identifier.as_ref().unwrap(), symbol, None);
                    }
                }
                Type::Record(Record {
                    ref name,
                    ref c_type,
                    ref functions,
                    ..
                })
                | Type::Class(Class {
                    ref name,
                    ref c_type,
                    ref functions,
                    ..
                })
                | Type::Interface(Interface {
                    ref name,
                    ref c_type,
                    ref functions,
                    ..
                }) => {
                    info.insert(c_type, symbol, Some(tid));
                    for func in functions {
                        let symbol = Symbol {
                            crate_name: crate_name.cloned(),
                            owner_name: Some(name.clone()),
                            name: func.name.clone(),
                        };
                        info.insert(func.c_identifier.as_ref().unwrap(), symbol, None);
                    }
                }
                _ => {}
            }
        }
    }

    info
}

impl Info {
    pub fn by_c_name(&self, name: &str) -> Option<&Symbol> {
        self.c_name_index
            .get(name)
            .map(|&id| &self.symbols[id as usize])
    }

    pub fn by_c_name_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        if let Some(&id) = self.c_name_index.get(name) {
            Some(&mut self.symbols[id as usize])
        } else {
            None
        }
    }

    pub fn by_tid(&self, tid: TypeId) -> Option<&Symbol> {
        self.tid_index
            .get(&Some(tid))
            .map(|&id| &self.symbols[id as usize])
    }

    fn insert(&mut self, name: &str, symbol: Symbol, tid: Option<TypeId>) {
        let id = self.symbols.len();
        self.symbols.push(symbol);
        self.c_name_index.insert(name.to_owned(), id as u32);
        if tid.is_some() {
            self.tid_index.insert(tid, id as u32);
        }
    }
}
