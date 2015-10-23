use std::collections::HashMap;
use std::ops::Deref;

use library;
use ns_vec::{self, NsVec};
use super::namespaces::{self, NsId};

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeDefId(ns_vec::Id);

impl Deref for TypeDefId {
    type Target = ns_vec::Id;

    fn deref(&self) -> &ns_vec::Id {
        &self.0
    }
}

impl From<ns_vec::Id> for TypeDefId {
    fn from(val: ns_vec::Id) -> TypeDefId {
        TypeDefId(val)
    }
}

/*
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FTypeStatus {
    Pending,
    Ignore,
    Ok,
}

impl Default for FTypeStatus {
    fn default() -> Self {
        FTypeStatus::Pending
    }
}
*/

#[derive(Debug, Default)]
pub struct TypeDef {
    name: String,
    gir_tid: library::TypeId,
}

#[derive(Debug)]
pub enum Ptr {
    Const,
    Mut,
}

pub type Ptrs = Vec<Ptr>;

#[derive(Debug)]
pub enum FType {
    Primitive,
    Alias(Ptrs, TypeDefId),
}

pub struct Info {
    data: NsVec<TypeDefId, TypeDef>,
    gir_tid_index: HashMap<library::TypeId, TypeDefId>,
}

pub fn run(gir: &library::Library, namespaces: &namespaces::Info, ) -> Info {
    let mut data = NsVec::new(namespaces.len());
    let mut gir_tid_index = HashMap::new();

    let mut info = Info {
        data: data,
        gir_tid_index: gir_tid_index,
    };

    for (ns_id, gir_ns) in gir.namespaces.iter().enumerate().skip(1) {
        let ns_id = ns_id as NsId;
        for id in 0..gir_ns.types.len() {
            let gir_tid = library::TypeId { ns_id: ns_id, id: id as u32 };
            analyze_gir_type(&mut info, gir, gir_tid);
        }
    }

    //analyze(&mut info, gir);

    info
}

fn analyze_gir_type(info: &mut Info, gir: &library::Library, gir_tid: library::TypeId) {
    use library::Type::*;
    let typ = gir.type_(gir_tid);
    let name = match *typ {
        Alias(ref x) => x.c_identifier.clone(),
        Bitfield(ref x) => x.c_type.clone(),
        Class(ref x) => x.c_type.clone(),
        Enumeration(ref x) => x.c_type.clone(),
        Function(library::Function { ref name, ref c_identifier, .. }) => {
            c_identifier.as_ref().unwrap_or(name).clone()
        }
        Interface(ref x) => x.c_type.clone(),
        Record(ref x) => x.c_type.clone(),
        Union(library::Union { ref name, ref c_type, .. }) => {
            c_type.as_ref().unwrap_or(name).clone()
        }
        _ => {
            warn!("Can't copy type `{:?}`", typ);
            return;
        }
    };
    trace!("Adding `{}`", name);
    let ftypedef = TypeDef {
        name: name,
        gir_tid: gir_tid,
        ..Default::default()
    };
    push(info, gir_tid.ns_id, ftypedef);
}

fn push(info: &mut Info, ns_id: NsId, ftypedef: TypeDef) -> TypeDefId {
    let gir_tid = ftypedef.gir_tid;
    let fid = info.data.push(ns_id, ftypedef);
    info.gir_tid_index.insert(gir_tid, fid);
    fid
}

//fn analyze(info: &mut Info, gir: &library::Library) {
//}
