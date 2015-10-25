use std::collections::{HashMap, VecDeque};
use std::ops::Deref;

use library;
use nameutil;
use ns_vec::{self, NsVec};
use super::namespaces::{self, NsId};
use traits::*;

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
    gir_tid: Option<library::TypeId>,
    type_: Type,
    ignore: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ptr {
    Const,
    Mut,
}

#[derive(Clone, Debug, Default)]
pub struct Ptrs(pub Vec<Ptr>);

impl Ptrs {
    pub fn none() -> Ptrs {
        Ptrs(vec![])
    }

    pub fn const_() -> Ptrs {
        Ptrs(vec![Ptr::Const])
    }

    pub fn mut_() -> Ptrs {
        Ptrs(vec![Ptr::Mut])
    }

    pub fn from_c_type(c_type: &str) -> Ptrs {
        let mut input = c_type.trim();
        let leading_const = input.starts_with("const ");
        if leading_const {
            input = &input[6..];
        }
        let end = [
                input.find(" const"),
                input.find("*const"),
                input.find("*"),
                Some(input.len()),
            ].iter()
            .filter_map(|&x| x)
            .min().unwrap();
        let inner = input[..end].trim();
        let mut ptrs = input[end..].rsplit('*').skip(1)
            .map(|s| if s.contains("const") { Ptr::Const } else { Ptr::Mut })
            .collect::<Vec<_>>();
        if let (true, Some(p)) = (leading_const, ptrs.last_mut()) {
            *p = Ptr::Const;
        }
        if inner == "gconstpointer" {
            ptrs.push(Ptr::Const);
        }
        else if inner == "gpointer" {
            ptrs.push(Ptr::Mut);
        }
        Ptrs(ptrs)
    }
}

#[derive(Debug)]
pub enum TypeRef {
    Void(Ptrs),
    Boolean(Ptrs),
    Int8(Ptrs),
    UInt8(Ptrs),
    Int16(Ptrs),
    UInt16(Ptrs),
    Int32(Ptrs),
    UInt32(Ptrs),
    Int64(Ptrs),
    UInt64(Ptrs),
    Char(Ptrs),
    UChar(Ptrs),
    Short(Ptrs),
    UShort(Ptrs),
    Int(Ptrs),
    UInt(Ptrs),
    Long(Ptrs),
    ULong(Ptrs),
    Size(Ptrs),
    SSize(Ptrs),
    Float(Ptrs),
    Double(Ptrs),
    Type(Ptrs),
    Id(Ptrs, TypeDefId),
    Function,
}

impl TypeRef {
    fn primitive(typ: &library::Type, ptrs: &Ptrs) -> Option<TypeRef> {
        if let library::Type::Fundamental(fund) = *typ {
            match fund {
                library::Fundamental::None => Some(TypeRef::Void(ptrs.clone())),
                library::Fundamental::Boolean => Some(TypeRef::Boolean(ptrs.clone())),
                library::Fundamental::Int8 => Some(TypeRef::Int8(ptrs.clone())),
                library::Fundamental::UInt8 => Some(TypeRef::UInt8(ptrs.clone())),
                library::Fundamental::Int16 => Some(TypeRef::Int16(ptrs.clone())),
                library::Fundamental::UInt16 => Some(TypeRef::UInt16(ptrs.clone())),
                library::Fundamental::Int32 => Some(TypeRef::Int32(ptrs.clone())),
                library::Fundamental::UInt32 => Some(TypeRef::UInt32(ptrs.clone())),
                library::Fundamental::Int64 => Some(TypeRef::Int64(ptrs.clone())),
                library::Fundamental::UInt64 => Some(TypeRef::UInt64(ptrs.clone())),
                library::Fundamental::Char => Some(TypeRef::Char(ptrs.clone())),
                library::Fundamental::UChar => Some(TypeRef::UChar(ptrs.clone())),
                library::Fundamental::Short => Some(TypeRef::Short(ptrs.clone())),
                library::Fundamental::UShort => Some(TypeRef::UShort(ptrs.clone())),
                library::Fundamental::Int => Some(TypeRef::Int(ptrs.clone())),
                library::Fundamental::UInt => Some(TypeRef::UInt(ptrs.clone())),
                library::Fundamental::Long => Some(TypeRef::Long(ptrs.clone())),
                library::Fundamental::ULong => Some(TypeRef::ULong(ptrs.clone())),
                library::Fundamental::Size => Some(TypeRef::Size(ptrs.clone())),
                library::Fundamental::SSize => Some(TypeRef::SSize(ptrs.clone())),
                library::Fundamental::Float => Some(TypeRef::Float(ptrs.clone())),
                library::Fundamental::Double => Some(TypeRef::Double(ptrs.clone())),
                library::Fundamental::Pointer => Some(TypeRef::Void(ptrs.clone())),
                library::Fundamental::Type => Some(TypeRef::Type(ptrs.clone())),
                _ => None,
            }
        }
        else {
            None
        }
    }
}

impl Default for TypeRef {
    fn default() -> TypeRef {
        TypeRef::Void(Ptrs::none())
    }
}

#[derive(Debug, Default)]
pub struct Field {
    name: String,
    type_ref: TypeRef,
    fake: bool,
    incomplete_self_reference
}

#[derive(Debug)]
pub enum Type {
    Alias(TypeRef),
    Function,
    Record {
        fields: Vec<Field>,
    },
}

impl Default for Type {
    fn default() -> Type {
        Type::Alias(TypeRef::Void(Ptrs::none()))
    }
}

pub struct Info {
    data: NsVec<TypeDefId, TypeDef>,
    gir_tid_index: HashMap<library::TypeId, TypeDefId>,
    name_index: HashMap<String, TypeDefId>,
    queue: VeqDeque<library::TypeId>,
}

struct Env<'a> {
    gir: &'a library::Library,
    namespaces: &'a namespaces::Info,
}

pub fn run(gir: &library::Library, namespaces: &namespaces::Info) -> Info {
    let mut info = Info {
        data: NsVec::new(namespaces.len()),
        gir_tid_index: HashMap::new(),
        name_index: HashMap::new(),
        queue: VecDeque::new(),
    };

    let env = Env {
        gir: gir,
        namespaces: namespaces,
    };

    for (ns_id, gir_ns) in env.gir.namespaces.iter().enumerate().skip(1) {
        let ns_id = ns_id as NsId;
        for id in 0..gir_ns.types.len() {
            info.queue.push_back(library::TypeId { ns_id: ns_id, id: id as u32 });
        }
    }
    
    while Some(gir_tid) = info.queue.pop_front() {
        analyze_gir_type(&mut info, &env, gir_tid);
    }

    //analyze(&mut info, &env);

    info
}

fn analyze_gir_type(info: &mut Info, env: &Env, gir_tid: library::TypeId)
        -> Option<TypeDefId> {
    use library::Type::*;
    let typ = env.gir.type_(gir_tid);
    let mut typedef = match *typ {
        Alias(ref alias) => analyze_gir_alias(info, env, alias),
        Function(library::Function { ref name, ref c_identifier, .. }) => {
            let name = c_identifier.as_ref().unwrap_or(name);
            TypeDef {
                name: name.clone(),
                type_: Type::Function,
                ..Default::default()
            }
        }
        Record(ref record) => analyze_gir_record(info, env, record),
        SList(..) => {
            trace!("SList");
            if let Some(&def_id) = info.name_index.get("GSList") {
                return Some(def_id);
            }
            let tid = env.gir.find_type(0, "GLib.SList").unwrap();
            analyze_gir_record(info, env, env.gir.type_(tid).to_ref_as())
        }
        /*
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
        */
        _ => {
            //warn!("Can't copy type `{:?}`", typ);
            return None;
        }
    };
    typedef.gir_tid = Some(gir_tid);
    trace!("Adding `{:?}`", typedef);
    Some(push(info, gir_tid.ns_id, typedef))
}

fn lookup_or_analyze_gir_type(info: &mut Info, env: &Env, gir_tid: library::TypeId)
        -> Option<TypeDefId> {
    info.gir_tid_index.get(&gir_tid).cloned()
        .or_else(|| analyze_gir_type(info, env, gir_tid))
}

fn type_ref(info: &mut Info, env: &Env, gir_tid: library::TypeId, c_type_hint: &str)
        -> Option<TypeRef> {
    let gir_type = env.gir.type_(gir_tid);
    let ptrs = Ptrs::from_c_type(c_type_hint);
    let ret = TypeRef::primitive(gir_type, &ptrs)
        .or_else(|| {
            lookup_or_analyze_gir_type(info, env, gir_tid)
                .map(|def_id| TypeRef::Id(ptrs, def_id))
        });
    if ret.is_none() {
        warn!("Failed to translate `{:?}` with hint `{}`", gir_type, c_type_hint);
    }
    ret
}

fn analyze_gir_alias(info: &mut Info, env: &Env, alias: &library::Alias) -> TypeDef {
    let type_ref = type_ref(info, env, alias.typ, &alias.target_c_type);
    let ignore = type_ref.is_none();
    TypeDef {
        name: alias.c_identifier.clone(),
        type_: Type::Alias(type_ref.unwrap_or(TypeRef::default())),
        ignore: ignore,
        ..Default::default()
    }
}

fn analyze_gir_record(info: &mut Info, env: &Env, record: &library::Record)
        -> TypeDef {
    let mut fields: Vec<Field> = Vec::new();
    let mut ignore = false;

    for field in &record.fields {
        match field.typ {
            library::FieldType::Type(field_tid, Some(ref c_type_hint)) => {
                if let Some(type_ref) = type_ref(info, env, field_tid, c_type_hint) {
                    fields.push(Field {
                        name: nameutil::mangle_keywords(&*field.name).into_owned(),
                        type_ref: type_ref,
                        ..Default::default()
                    });
                }
                else {
                    warn!("Failed to translate the field `{:?}` from `{:?}`", field, record);
                    ignore = true;
                    //break;
                }
            }
            library::FieldType::Type(field_tid, None) => {
                warn!("Failed to translate the field `{:?}` from `{:?}`", field, record);
                ignore = true;
                //break;
            }
            library::FieldType::Function(..) => {
                fields.push(Field {
                    name: nameutil::mangle_keywords(&*field.name).into_owned(),
                    type_ref: TypeRef::Function,
                    ..Default::default()
                });
                //break;
            }
            library::FieldType::Union(..) => {
                warn!("Failed to translate the field `{:?}` from `{:?}`", field, record);
                ignore = true;
                //break;
            }
        }
    }

    TypeDef {
        name: record.c_type.clone(),
        type_: Type::Record {
            fields: fields,
        },
        ignore: ignore,
        ..Default::default()
    }
}

fn push(info: &mut Info, ns_id: NsId, type_def: TypeDef) -> TypeDefId {
    let gir_tid = type_def.gir_tid;
    let name = type_def.name.clone();
    let fid = info.data.push(ns_id, type_def);
    if let Some(gir_tid) = gir_tid {
        info.gir_tid_index.insert(gir_tid, fid);
    }
    info.name_index.insert(name, fid);
    fid
}


//fn analyze(info: &mut Info, env: &Env) {
//}
