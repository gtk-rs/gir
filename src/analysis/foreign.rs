use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use std::ops::Deref;

use library;
use nameutil;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decorator {
    ConstPtr,
    MutPtr,
    Volatile,
    FixedArray(u16),
}

#[derive(Clone, Debug, Default)]
pub struct Decorators(pub Vec<Decorator>);

impl Decorators {
    pub fn none() -> Decorators {
        Decorators(vec![])
    }

    pub fn const_ptr() -> Decorators {
        Decorators(vec![Decorator::ConstPtr])
    }

    pub fn mut_ptr() -> Decorators {
        Decorators(vec![Decorator::MutPtr])
    }

    pub fn fixed_array(length: u16) -> Decorators {
        Decorators(vec![Decorator::FixedArray(length)])
    }

    pub fn from_c_type(c_type: &str) -> Decorators {
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
            .map(|s| if s.contains("const") { Decorator::ConstPtr } else { Decorator::MutPtr })
            .collect::<Vec<_>>();
        if let (true, Some(p)) = (leading_const, ptrs.last_mut()) {
            *p = Decorator::ConstPtr;
        }
        if inner == "gconstpointer" {
            ptrs.push(Decorator::ConstPtr);
        }
        else if inner == "gpointer" {
            ptrs.push(Decorator::MutPtr);
        }
        Decorators(ptrs)
    }
}

#[derive(Debug)]
pub enum TypeTerminal {
    Void,
    Boolean,
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Char,
    UChar,
    Short,
    UShort,
    Int,
    UInt,
    Long,
    ULong,
    Size,
    SSize,
    Float,
    Double,
    Type,
    Function,
    Id(TypeDefId),
    Postponed(library::TypeId),
}

#[derive(Debug, Default)]
pub struct TypeRef {
    pub decorators: Decorators,
    pub type_terminal: TypeTerminal,
}

impl TypeTerminal {
    fn primitive(typ: &library::Type) -> Option<TypeTerminal> {
        if let library::Type::Fundamental(fund) = *typ {
            match fund {
                library::Fundamental::None => Some(TypeTerminal::Void),
                library::Fundamental::Boolean => Some(TypeTerminal::Boolean),
                library::Fundamental::Int8 => Some(TypeTerminal::Int8),
                library::Fundamental::UInt8 => Some(TypeTerminal::UInt8),
                library::Fundamental::Int16 => Some(TypeTerminal::Int16),
                library::Fundamental::UInt16 => Some(TypeTerminal::UInt16),
                library::Fundamental::Int32 => Some(TypeTerminal::Int32),
                library::Fundamental::UInt32 => Some(TypeTerminal::UInt32),
                library::Fundamental::Int64 => Some(TypeTerminal::Int64),
                library::Fundamental::UInt64 => Some(TypeTerminal::UInt64),
                library::Fundamental::Char => Some(TypeTerminal::Char),
                library::Fundamental::UChar => Some(TypeTerminal::UChar),
                library::Fundamental::Short => Some(TypeTerminal::Short),
                library::Fundamental::UShort => Some(TypeTerminal::UShort),
                library::Fundamental::Int => Some(TypeTerminal::Int),
                library::Fundamental::UInt => Some(TypeTerminal::UInt),
                library::Fundamental::Long => Some(TypeTerminal::Long),
                library::Fundamental::ULong => Some(TypeTerminal::ULong),
                library::Fundamental::Size => Some(TypeTerminal::Size),
                library::Fundamental::SSize => Some(TypeTerminal::SSize),
                library::Fundamental::Float => Some(TypeTerminal::Float),
                library::Fundamental::Double => Some(TypeTerminal::Double),
                library::Fundamental::Pointer => Some(TypeTerminal::Void),
                library::Fundamental::Type => Some(TypeTerminal::Type),
                library::Fundamental::Utf8 => Some(TypeTerminal::Char),
                _ => None,
            }
        }
        else {
            None
        }
    }
}

impl Default for TypeTerminal {
    fn default() -> TypeTerminal {
        TypeTerminal::Void
    }
}

#[derive(Debug, Default)]
pub struct Field {
    name: String,
    type_ref: TypeRef,
    fake: bool,
}

#[derive(Debug)]
pub enum Type {
    Alias(TypeRef),
    Bitfield,
    Enumeration,
    Function,
    Opaque,
    Record {
        fields: Vec<Field>,
        is_class: bool,
    },
    Union {
        fields: Vec<Field>,
    },
}

impl Default for Type {
    fn default() -> Type {
        Type::Alias(Default::default())
    }
}

#[derive(Debug, Default)]
pub struct TypeDef {
    name: String,
    gir_tid: Option<library::TypeId>,
    type_: Type,
    ignore: Option<bool>,
}

pub struct Info {
    data: NsVec<TypeDefId, TypeDef>,
    gir_tid_index: HashMap<library::TypeId, TypeDefId>,
    name_index: HashMap<String, TypeDefId>,
    queue: VecDeque<library::TypeId>,
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

    enqueue_glib_containers(&mut info, &env);
    enqueue_gir_namespaces(&mut info, &env);

    while let Some(gir_tid) = info.queue.pop_front() {
        if info.gir_tid_index.get(&gir_tid).is_none() {
            transfer_gir_type(&mut info, &env, gir_tid);
        }
    }

    resolve_postponed_types(&mut info, &env);

    //analyze(&mut info, &env);

    info
}

fn enqueue_glib_containers(info: &mut Info, env: &Env) {
    let names = [
        "GLib.Array",
        "GLib.ByteArray",
        "GLib.PtrArray",
        "GLib.HashTable",
        "GLib.List",
        "GLib.SList",
    ];
    for name in names.iter() {
        let tid = env.gir.find_type(0, name).expect("Missing GLib built-in type");
        info.queue.push_back(tid);
    }
}

fn enqueue_gir_namespaces(info: &mut Info, env: &Env) {
    for (ns_id, gir_ns) in env.gir.namespaces.iter().enumerate().skip(1) {
        let ns_id = ns_id as NsId;
        for id in 0..gir_ns.types.len() {
            info.queue.push_back(library::TypeId { ns_id: ns_id, id: id as u32 });
        }
    }
}

fn transfer_gir_type(info: &mut Info, env: &Env, gir_tid: library::TypeId) {
    use library::Type::*;
    let typ = env.gir.type_(gir_tid);
    let mut typedef = match *typ {
        Alias(library::Alias { ref c_identifier, typ, ref target_c_type, .. }) => {
            TypeDef {
                name: c_identifier.clone(),
                type_: Type::Alias(make_type_ref(info, env, typ, target_c_type)),
                ..Default::default()
            }
        }
        Bitfield(library::Bitfield { ref c_type, .. }) => {
            TypeDef {
                name: c_type.clone(),
                type_: Type::Bitfield,
                ..Default::default()
            }
        }
        Enumeration(library::Enumeration { ref c_type, .. }) => {
            TypeDef {
                name: c_type.clone(),
                type_: Type::Enumeration,
                ..Default::default()
            }
        }
        Function(library::Function { ref name, ref c_identifier, .. }) => {
            let name = c_identifier.as_ref().unwrap_or(name);
            TypeDef {
                name: name.clone(),
                type_: Type::Function,
                ..Default::default()
            }
        }
        Interface(library::Interface { ref c_type, .. }) => {
            TypeDef {
                name: c_type.clone(),
                type_: Type::Opaque,
                ..Default::default()
            }
        }
        Class(ref class) => transfer_gir_class(info, env, gir_tid.ns_id, class),
        Record(ref record) => transfer_gir_record(info, env, gir_tid.ns_id, record),
        Union(ref union) => transfer_gir_union(info, env, gir_tid.ns_id, union),
        Array(..) => {
            info.gir_tid_index.insert(gir_tid, info.name_index.get("GArray").cloned().unwrap());
            return;
        }
        /*
        ByteArray(..) => {
            info.gir_tid_index.insert(gir_tid, info.name_index.get("GByteArray").cloned().unwrap());
            return;
        }
        */
        PtrArray(..) => {
            info.gir_tid_index.insert(gir_tid, info.name_index.get("GPtrArray").cloned().unwrap());
            return;
        }
        HashTable(..) => {
            info.gir_tid_index.insert(gir_tid, info.name_index.get("GHashTable").cloned().unwrap());
            return;
        }
        List(..) => {
            info.gir_tid_index.insert(gir_tid, info.name_index.get("GList").cloned().unwrap());
            return;
        }
        SList(..) => {
            info.gir_tid_index.insert(gir_tid, info.name_index.get("GSList").cloned().unwrap());
            return;
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
            info!("Can't copy type `{:?}`", typ);
            return;
        }
    };
    typedef.gir_tid = Some(gir_tid);
    trace!("Adding `{:?}`", typedef);
    push(info, gir_tid.ns_id, typedef);
}

fn make_type_ref(info: &mut Info, env: &Env, gir_tid: library::TypeId, c_type_hint: &str)
        -> TypeRef {
    let gir_type = env.gir.type_(gir_tid);
    let decs = Decorators::from_c_type(c_type_hint);
    if let Some(term) = TypeTerminal::primitive(gir_type) {
        TypeRef {
            decorators: decs,
            type_terminal: term,
            ..Default::default
        }
    }
    else if let Some(&def_id) = info.gir_tid_index.get(&gir_tid) {
        TypeRef {
            decorators: decs,
            type_terminal: TypeTerminal::Id(def_id),
            ..Default::default
        }
    }
    else if let library::Type::CArray(tid) = *gir_type {
        let TypeRef { type_terminal, .. } = make_type_ref(info, env, tid, "");
        TypeRef {
            decorators: Decorators::mut_ptr(),
            type_terminal: type_terminal,
            ..Default::default
        }
    }
    else if let library::Type::FixedArray(tid, length) = *gir_type {
        let TypeRef { type_terminal, .. } = make_type_ref(info, env, tid, "");
        TypeRef {
            decorators: Decorators::fixed_array(length),
            type_terminal: type_terminal,
            ..Default::default
        }
    }
    else {
        info.queue.push_back(gir_tid);
        TypeRef {
            decorators: decs,
            type_terminal: TypeTerminal::Postponed(gir_tid),
            ..Default::default
        }
    }
}

fn transfer_gir_record(info: &mut Info, env: &Env, ns_id: NsId, record: &library::Record)
        -> TypeDef {
    let name = record.c_type.as_ref().unwrap_or(&record.name).clone();
    transfer_gir_recordlike(info, env, ns_id, name, &record.fields, false, record)
}

fn transfer_gir_class(info: &mut Info, env: &Env, ns_id: NsId, class: &library::Class)
        -> TypeDef {
    let name = class.c_type.clone();
    transfer_gir_recordlike(info, env, ns_id, name, &[], true, class)
}

fn transfer_gir_recordlike(info: &mut Info, env: &Env, ns_id: NsId, name: String,
        record_fields: &[library::Field], is_class: bool, record: &Debug) -> TypeDef {
    let mut fields: Vec<Field> = Vec::new();
    let mut bits: Option<u8> = None;
    let mut bit_names: Vec<&str> = vec![];
    //let mut ignore = false;

    for field in record_fields {
        if let Some(more_bits) = field.bits {
            bits = Some(bits.unwrap_or(0) + more_bits);
            bit_names.push(&field.name);
            continue;
        }
        if let Some(bits) = bits.take() {
            let bytes = (bits + 7) / 8;
            fields.push(Field {
                name: nameutil::mangle_keywords(&*bit_names.join("__")).into_owned(),
                type_ref: TypeRef {
                    decorators: Decorators::fixed_array(bytes as u16),
                    type_terminal: TypeTerminal::UInt8,
                    ..Default::default()
                },
                fake: true,
                ..Default::default()
            });
            bit_names.clear();
        }
        match *field {
            library::Field { typ, c_type: Some(ref c_type_hint), .. } => {
                fields.push(Field {
                    name: nameutil::mangle_keywords(&*field.name).into_owned(),
                    type_ref: make_type_ref(info, env, typ, c_type_hint),
                    ..Default::default()
                });
            }
            library::Field { typ, .. } if typ.ns_id == namespaces::INTERNAL => {
                match *env.gir.type_(typ) {
                    library::Type::Function(..) => {
                        fields.push(Field {
                            name: nameutil::mangle_keywords(&*field.name).into_owned(),
                            type_ref: TypeRef {
                                decorators: Decorators::none(),
                                type_terminal: TypeTerminal::Function,
                                ..Default::default()
                            }
                            ..Default::default()
                        });
                    }
                    library::Type::Union(ref union) => {
                        let mut def = transfer_gir_union(info, env, ns_id, union);
                        def.name = format!("{}_{}", name, field.name);
                        def.gir_tid = Some(typ);
                        trace!("Adding `{:?}`", def);
                        let def_id = push(info, ns_id, def);
                        fields.push(Field {
                            name: nameutil::mangle_keywords(&*field.name).into_owned(),
                            type_ref: TypeRef {
                                decorators: Decorators::none(),
                                type_terminal: TypeTerminal::Id(def_id),
                                ..Default::default()
                            }
                            ..Default::default()
                        });
                    }
                    _ => {
                        warn!("Failed to translate the field `{:?}` from `{:?}`", field, record);
                    }
                }
            }
            library::Field { typ, c_type: None, .. } => {
                // seems harmless
                //warn!("Missing c:type for field `{:?}` from `{:?}`", field, record);
                fields.push(Field {
                    name: nameutil::mangle_keywords(&*field.name).into_owned(),
                    type_ref: make_type_ref(info, env, typ, ""),
                    ..Default::default()
                });
            }
        }
    }

    TypeDef {
        name: name,
        type_: Type::Record {
            fields: fields,
            is_class: is_class,
        },
        //ignore: ignore,
        ..Default::default()
    }
}

fn transfer_gir_union(info: &mut Info, env: &Env, ns_id: NsId, union: &library::Union) -> TypeDef {
    let mut fields: Vec<Field> = Vec::new();
    let name = union.c_type.as_ref().unwrap_or(&union.name).clone();
    //let mut ignore = false;

    for field in &union.fields {
        match *field {
            library::Field { typ, c_type: Some(ref c_type_hint), .. } => {
                fields.push(Field {
                    name: nameutil::mangle_keywords(&*field.name).into_owned(),
                    type_ref: make_type_ref(info, env, typ, c_type_hint),
                    ..Default::default()
                });
            }
            library::Field { typ, .. } if typ.ns_id == namespaces::INTERNAL => {
                match *env.gir.type_(typ) {
                    library::Type::Record(ref record) => {
                        let mut def = transfer_gir_record(info, env, ns_id, record);
                        def.name = format!("{}_{}", name, field.name);
                        def.gir_tid = Some(typ);
                        trace!("Adding `{:?}`", def);
                        let def_id = push(info, ns_id, def);
                        fields.push(Field {
                            name: nameutil::mangle_keywords(&*field.name).into_owned(),
                            type_ref: TypeRef {
                                decorators: Decorators::none(),
                                type_terminal: TypeTerminal::Id(def_id),
                                ..Default::default()
                            }
                            ..Default::default()
                        });
                    }
                    _ => {
                        warn!("Failed to translate the field `{:?}` from `{:?}`", field, union);
                    }
                }
            }
            _ => {
                warn!("Failed to translate the field `{:?}` from `{:?}`", field, union);
                //ignore = true;
                //break;
            }
        }
    }

    TypeDef {
        name: name,
        type_: Type::Union {
            fields: fields,
        },
        //ignore: ignore,
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


fn resolve_postponed_types(info: &mut Info, env: &Env) {
    for ns_id in 0..env.namespaces.len() as NsId {
        for def_id in info.data.ids_by_ns(ns_id) {
            let TypeDef { ref mut type_, ref mut ignore, .. } = info.data[def_id];
            match *type_ {
                Type::Alias(ref mut type_ref) => {
                    resolve(&info.gir_tid_index, env, type_ref, ignore);
                }
                Type::Record { ref mut fields, .. } => {
                    for field in fields.iter_mut() {
                        resolve(&info.gir_tid_index, env, &mut field.type_ref,
                            ignore);
                    }
                }
                _ => {}
            }
        }
    }
}

fn resolve(gir_tid_index: &HashMap<library::TypeId, TypeDefId>, env: &Env, type_ref: &mut TypeRef,
        ignore: &mut Option<bool>) {
    if let TypeTerminal::Postponed(gir_tid) = *type_ref.type_terminal {
        if let Some(&def_id) = gir_tid_index.get(&gir_tid) {
            trace!("Resolved `{:?}` to `{:?}`", gir_tid, def_id);
            *type_ref.type_terminal = TypeTerminal::Id(def_id);
        }
        else {
            info!("Couldn't resolve `{:?}`", env.gir.type_(gir_tid));
            *ignore = Some(true);
        }
    }
}

//fn analyze(info: &mut Info, env: &Env) {
//}
