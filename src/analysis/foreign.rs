use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use std::mem;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Decorator {
    ConstPtr,
    MutPtr,
    Volatile,
    FixedArray(u16),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
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

    pub fn to_rust(&self, name: &str) -> String {
        use self::Decorator::*;
        let mut ret = String::from(name);
        for dec in self.0.iter().rev() {
            match *dec {
                ConstPtr => ret = format!("*const {}", ret),
                MutPtr => ret = format!("*mut {}", ret),
                Volatile => ret = format!("Volatile<{}>", ret),
                FixedArray(length) => ret = format!("[{}; {}]", ret, length),
            }
        }
        ret
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

impl TypeTerminal {
    fn to_rust<'a>(&self, info: &'a Info, env: &Env) -> (&'a str, Cow<str>) {
        use self::TypeTerminal::*;
        match *self {
            Void => ("c_void", Cow::from("c_void")),
            Boolean => ("gboolean", Cow::from("gboolean")),
            Int8 => ("i8", Cow::from("i8")),
            UInt8 => ("u8", Cow::from("u8")),
            Int16 => ("i16", Cow::from("i16")),
            UInt16 => ("u16", Cow::from("u16")),
            Int32 => ("i32", Cow::from("i32")),
            UInt32 => ("u32", Cow::from("u32")),
            Int64 => ("i64", Cow::from("i64")),
            UInt64 => ("u64", Cow::from("u64")),
            Char => ("c_char", Cow::from("c_char")),
            UChar => ("c_uchar", Cow::from("c_uchar")),
            Short => ("c_short", Cow::from("c_short")),
            UShort => ("c_ushort", Cow::from("c_ushort")),
            Int => ("c_int", Cow::from("c_int")),
            UInt => ("c_uint", Cow::from("c_uint")),
            Long => ("c_long", Cow::from("c_long")),
            ULong => ("c_ulong", Cow::from("c_ulong")),
            Size => ("size_t", Cow::from("size_t")),
            SSize => ("ssize_t", Cow::from("ssize_t")),
            Float => ("c_float", Cow::from("c_float")),
            Double => ("c_double", Cow::from("c_double")),
            Type => ("GType", Cow::from("GType")),
            Function => ("fn()", Cow::from("fn()")),
            Id(def_id) => {
                let name = &info.data[def_id].name;
                let external_name =
                    Cow::from(format!("{}::{}", env.namespaces[def_id.ns_id].name, name));
                (name, external_name)
            }
            Postponed(..) => {
                ("c_void /* error */", Cow::from("c_void /* error */"))
            }
        }
    }
}

impl Default for TypeTerminal {
    fn default() -> TypeTerminal {
        TypeTerminal::Void
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TypeRef(Decorators, TypeTerminal);

impl TypeRef {
    fn to_rust(&self, info: &Info, env: &Env) -> (String, String) {
        let TypeRef(ref decorators, ref type_terminal) = *self;
        let (name, external_name) = type_terminal.to_rust(info, env);
        (decorators.to_rust(name), decorators.to_rust(&*external_name))
    }
}

#[derive(Debug, Default)]
pub struct Function;

#[derive(Debug)]
pub enum FieldType {
    Ref(TypeRef),
    Function(Function),
}

impl Default for FieldType {
    fn default() -> FieldType {
        FieldType::Ref(TypeRef::default())
    }
}

#[derive(Debug, Default)]
pub struct Field {
    pub name: String,
    pub type_: FieldType,
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
        fake: bool,
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
    pub name: String,
    gir_tid: Option<library::TypeId>,
    pub type_: Type,
    pub ignore: Option<bool>,
    pub public: bool,
}

pub struct Info {
    pub data: NsVec<TypeDefId, TypeDef>,
    gir_tid_index: HashMap<library::TypeId, TypeDefId>,
    name_index: HashMap<String, TypeDefId>,
    queue: VecDeque<library::TypeId>,
    pub rust_type: HashMap<TypeRef, String>,
    pub rust_type_external: HashMap<TypeRef, String>,
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
        rust_type: HashMap::new(),
        rust_type_external: HashMap::new(),
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
    fix_weird_types(&mut info);
    prepare_rust_types(&mut info, &env);

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
        _ => {
            info!("Can't copy type `{:?}`", typ);
            return;
        }
    };
    typedef.gir_tid = Some(gir_tid);
    push(info, gir_tid.ns_id, typedef);
}

fn make_type_ref(info: &mut Info, env: &Env, gir_tid: library::TypeId, c_type_hint: &str)
        -> TypeRef {
    let gir_type = env.gir.type_(gir_tid);
    let decorators = Decorators::from_c_type(c_type_hint);
    if let Some(type_terminal) = TypeTerminal::primitive(gir_type) {
        TypeRef(decorators, type_terminal)
    }
    else if let Some(&def_id) = info.gir_tid_index.get(&gir_tid) {
        TypeRef(decorators, TypeTerminal::Id(def_id))
    }
    else if let library::Type::CArray(tid) = *gir_type {
        let TypeRef(_, type_terminal) = make_type_ref(info, env, tid, "");
        TypeRef(Decorators::mut_ptr(), type_terminal)
    }
    else if let library::Type::FixedArray(tid, length) = *gir_type {
        let TypeRef(_, type_terminal) = make_type_ref(info, env, tid, "");
        TypeRef(Decorators::fixed_array(length), type_terminal)
    }
    else {
        info.queue.push_back(gir_tid);
        TypeRef(decorators, TypeTerminal::Postponed(gir_tid))
    }
}

fn transfer_gir_record(info: &mut Info, env: &Env, ns_id: NsId, record: &library::Record)
        -> TypeDef {
    let name = record.c_type.as_ref().unwrap_or(&record.name).clone();
    let mut def = transfer_gir_recordlike(info, env, ns_id, name, &record.fields, false, record);
    def.public = !record.disguised;
    def
}

fn transfer_gir_class(info: &mut Info, env: &Env, ns_id: NsId, class: &library::Class)
        -> TypeDef {
    let name = class.c_type.clone();
    transfer_gir_recordlike(info, env, ns_id, name, &[], true, class)
}

fn transfer_gir_recordlike(info: &mut Info, env: &Env, ns_id: NsId, name: String,
        record_fields: &[library::Field], is_class: bool, record: &Debug) -> TypeDef {
    fn flush_bits_placeholder(fields: &mut Vec<Field>, bits: u8, count: u8) {
        let bytes = (bits + 7) / 8;
        fields.push(Field {
            name: format!("bits{}", count),
            type_: FieldType::Ref(
                TypeRef(Decorators::fixed_array(bytes as u16), TypeTerminal::UInt8)),
            fake: true,
            ..Default::default()
        });
    }

    let mut fields: Vec<Field> = Vec::new();
    let mut bits: Option<u8> = None;
    let mut bits_placeholder_count = 0u8;
    //let mut ignore = false;

    for field in record_fields {
        if let Some(more_bits) = field.bits {
            bits = Some(bits.unwrap_or(0) + more_bits);
            continue;
        }
        if let Some(bits) = bits.take() {
            flush_bits_placeholder(&mut fields, bits, bits_placeholder_count);
            bits_placeholder_count += 1;
        }
        match *field {
            library::Field { typ, c_type: Some(ref c_type_hint), .. } => {
                fields.push(Field {
                    name: nameutil::mangle_keywords(&*field.name).into_owned(),
                    type_: FieldType::Ref(make_type_ref(info, env, typ, c_type_hint)),
                    ..Default::default()
                });
            }
            library::Field { typ, .. } if typ.ns_id == namespaces::INTERNAL => {
                match *env.gir.type_(typ) {
                    library::Type::Function(..) => {
                        fields.push(Field {
                            name: nameutil::mangle_keywords(&*field.name).into_owned(),
                            type_: FieldType::Function(Function),
                            ..Default::default()
                        });
                    }
                    library::Type::Union(ref union) => {
                        let mut def = transfer_gir_union(info, env, ns_id, union);
                        def.name = format!("{}_{}", name, field.name);
                        //def.fake = true;
                        def.gir_tid = Some(typ);
                        let def_id = push(info, ns_id, def);
                        fields.push(Field {
                            name: nameutil::mangle_keywords(&*field.name).into_owned(),
                            type_: FieldType::Ref(
                                TypeRef(Decorators::none(), TypeTerminal::Id(def_id))),
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
                    type_: FieldType::Ref(make_type_ref(info, env, typ, "")),
                    ..Default::default()
                });
            }
        }
    }

    if let Some(bits) = bits.take() {
        flush_bits_placeholder(&mut fields, bits, bits_placeholder_count);
    }

    TypeDef {
        name: name,
        type_: Type::Record {
            fields: fields,
            is_class: is_class,
            fake: false,
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
                    type_: FieldType::Ref(make_type_ref(info, env, typ, c_type_hint)),
                    ..Default::default()
                });
            }
            library::Field { typ, .. } if typ.ns_id == namespaces::INTERNAL => {
                match *env.gir.type_(typ) {
                    library::Type::Record(ref record) => {
                        let mut def = transfer_gir_record(info, env, ns_id, record);
                        def.name = format!("{}_{}", name, field.name);
                        if let Type::Record { ref mut fake, .. } = def.type_ {
                            *fake = true;
                        }
                        def.gir_tid = Some(typ);
                        let def_id = push(info, ns_id, def);
                        fields.push(Field {
                            name: nameutil::mangle_keywords(&*field.name).into_owned(),
                            type_: FieldType::Ref(
                                TypeRef(Decorators::none(), TypeTerminal::Id(def_id))),
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
    trace!("Adding `{:?}`", type_def);
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
                        match field.type_ {
                            FieldType::Ref(ref mut type_ref) => {
                                resolve(&info.gir_tid_index, env, type_ref, ignore);
                            }
                            FieldType::Function(..) => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn resolve(gir_tid_index: &HashMap<library::TypeId, TypeDefId>, env: &Env, type_ref: &mut TypeRef,
        ignore: &mut Option<bool>) {
    let TypeRef(_, ref mut type_terminal) = *type_ref;
    if let TypeTerminal::Postponed(gir_tid) = *type_terminal {
        if let Some(&def_id) = gir_tid_index.get(&gir_tid) {
            trace!("Resolved `{:?}` to `{:?}`", gir_tid, def_id);
            *type_terminal = TypeTerminal::Id(def_id);
        }
        else {
            info!("Couldn't resolve `{:?}`", env.gir.type_(gir_tid));
            *ignore = Some(true);
        }
    }
}

fn prepare_rust_types(info: &mut Info, env: &Env) {
    for ns_id in 0..env.namespaces.len() as NsId {
        for def_id in info.data.ids_by_ns(ns_id) {
            let TypeDef { ref type_, .. } = info.data[def_id];
            match *type_ {
                Type::Alias(ref type_ref) => {
                    if let Some((s, s_ext)) = make_rust_type(info, env, type_ref) {
                        info.rust_type.insert(type_ref.clone(), s);
                        info.rust_type_external.insert(type_ref.clone(), s_ext);
                    }
                }
                Type::Record { ref fields, .. } => {
                    for field in fields {
                        match field.type_ {
                            FieldType::Ref(ref type_ref) => {
                                if let Some((s, s_ext)) = make_rust_type(info, env, type_ref) {
                                    info.rust_type.insert(type_ref.clone(), s);
                                    info.rust_type_external.insert(type_ref.clone(), s_ext);
                                }
                            }
                            FieldType::Function(..) => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn make_rust_type(info: &Info, env: &Env, type_ref: &TypeRef) -> Option<(String, String)> {
    if info.rust_type.get(type_ref).is_some() {
        None
    }
    else {
        Some(type_ref.to_rust(info, env))
    }
}

fn fix_weird_types(info: &mut Info) {
    fn atomize(info: &mut Info, name: &str) {
        if let Some(&def_id) = info.name_index.get(name) {
            let mut def = TypeDef {
                name: String::from(name),
                ..Default::default()
            };
            mem::swap(&mut def, &mut info.data[def_id]);
            def.name = format!("_{}", name);
            let new_def_id = push(info, def_id.ns_id, def);
            info.data[def_id].type_ =
                Type::Alias(TypeRef(Decorators::mut_ptr(), TypeTerminal::Id(new_def_id)));
        }
    }

    atomize(info, "GIConv");
}
