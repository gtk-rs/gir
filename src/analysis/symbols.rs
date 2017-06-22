use analysis::namespaces::{self, NsId};
use case::CaseExt;
use library::*;
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

    pub fn make_trait_method(&mut self) {
        let name = self.owner_name.take();
        self.owner_name = name.map(|s| format!("{}Ext", s));
    }
}

#[derive(Debug)]
pub struct Info {
    symbols: Vec<Symbol>,
    c_name_index: HashMap<String, u32>,
}

pub fn run(library: &Library, namespaces: &namespaces::Info) -> Info {
    let mut info = Info {
        symbols: Vec::new(),
        c_name_index: HashMap::new(),
    };

    info.insert(
        "NULL",
        Symbol {
            name: "None".into(),
            ..Default::default()
        },
    );
    info.insert(
        "FALSE",
        Symbol {
            name: "false".into(),
            ..Default::default()
        },
    );
    info.insert(
        "TRUE",
        Symbol {
            name: "true".into(),
            ..Default::default()
        },
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

        for typ in ns.types.iter().map(|t| t.as_ref().unwrap()) {
            let symbol = Symbol {
                crate_name: crate_name.cloned(),
                name: typ.get_name(),
                ..Default::default()
            };

            match *typ {
                Type::Alias(Alias { ref c_identifier, .. }) => {
                    info.insert(c_identifier, symbol);
                }
                Type::Enumeration(Enumeration {
                    ref name,
                    ref c_type,
                    ref members,
                    ref functions,
                    ..
                }) |
                Type::Bitfield(Bitfield {
                    ref name,
                    ref c_type,
                    ref members,
                    ref functions,
                    ..
                }) => {
                    info.insert(c_type, symbol);
                    for member in members {
                        let symbol = Symbol {
                            crate_name: crate_name.cloned(),
                            owner_name: Some(name.clone()),
                            name: member.name.to_camel(),
                        };
                        info.insert(&member.c_identifier, symbol);
                    }
                    for func in functions {
                        let symbol = Symbol {
                            crate_name: crate_name.cloned(),
                            owner_name: Some(name.clone()),
                            name: func.name.clone(),
                        };
                        info.insert(func.c_identifier.as_ref().unwrap(), symbol);
                    }
                }
                Type::Record(Record {
                    ref name,
                    ref c_type,
                    ref functions,
                    ..
                }) |
                Type::Class(Class {
                    ref name,
                    ref c_type,
                    ref functions,
                    ..
                }) |
                Type::Interface(Interface {
                    ref name,
                    ref c_type,
                    ref functions,
                    ..
                }) => {
                    info.insert(c_type, symbol);
                    for func in functions {
                        let symbol = Symbol {
                            crate_name: crate_name.cloned(),
                            owner_name: Some(name.clone()),
                            name: func.name.clone(),
                        };
                        info.insert(func.c_identifier.as_ref().unwrap(), symbol);
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

    fn insert(&mut self, name: &str, symbol: Symbol) {
        let id = self.symbols.len();
        self.symbols.push(symbol);
        self.c_name_index.insert(name.to_owned(), id as u32);
    }
}
