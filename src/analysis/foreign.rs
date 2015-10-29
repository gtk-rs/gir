use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
use std::mem;
use std::ops::Deref;

use env;
use library;
use nameutil;
use ns_vec::{self, NsVec};
use super::namespaces::{self, NsId};

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct DefId(ns_vec::Id);

impl Deref for DefId {
    type Target = ns_vec::Id;

    fn deref(&self) -> &ns_vec::Id {
        &self.0
    }
}

impl From<ns_vec::Id> for DefId {
    fn from(val: ns_vec::Id) -> DefId {
        DefId(val)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Decorator {
    ConstPtr,
    MutPtr,
    Option,
    Volatile,
    FixedArray(u16),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Decorators(pub Vec<Decorator>);

const CONST: &'static str = "const ";
const VOLATILE: &'static str = "volatile ";

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

    pub fn option() -> Decorators {
        Decorators(vec![Decorator::Option])
    }

    pub fn fixed_array(length: u16) -> Decorators {
        Decorators(vec![Decorator::FixedArray(length)])
    }

    pub fn from_c_type(c_type: &str) -> Decorators {
        let mut input = c_type.trim();
        let volatile = input.starts_with(VOLATILE);
        if volatile {
            input = &input[VOLATILE.len()..];
        }
        let leading_const = input.starts_with(CONST);
        if leading_const {
            input = &input[CONST.len()..];
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
        if volatile {
            ptrs.push(Decorator::Volatile);
        }
        if inner == "gconstpointer" {
            ptrs.push(Decorator::ConstPtr);
        }
        else if inner == "gpointer" {
            ptrs.push(Decorator::MutPtr);
        }
        Decorators(ptrs)
    }

    pub fn is_none(&self) -> bool {
        self.0.is_empty()
    }

    pub fn to_rust(&self, name: &str) -> String {
        use self::Decorator::*;
        let mut ret = String::from(name);
        for dec in self.0.iter().rev() {
            match *dec {
                ConstPtr => ret = format!("*const {}", ret),
                MutPtr => ret = format!("*mut {}", ret),
                Option => ret = format!("Option<{}>", ret),
                Volatile => ret = format!("Volatile<{}>", ret),
                FixedArray(length) => ret = format!("[{}; {}]", ret, length),
            }
        }
        ret
    }

    pub fn push_front(&mut self, other: &Decorators) {
        let mut cat = [&self.0[..], &other.0[..]].concat();
        mem::swap(&mut self.0, &mut cat);
    }

    pub fn push_back(&mut self, other: &Decorators) {
        let mut cat = [&other.0[..], &self.0[..]].concat();
        mem::swap(&mut self.0, &mut cat);
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
    Id(DefId),
    Postponed(library::TypeId),
}

impl TypeTerminal {
    fn primitive(typ: &library::Type) -> Option<TypeTerminal> {
        use library::Fundamental;
        use self::TypeTerminal::*;
        if let library::Type::Fundamental(fund) = *typ {
            match fund {
                Fundamental::None => Some(Void),
                Fundamental::Boolean => Some(Boolean),
                Fundamental::Int8 => Some(Int8),
                Fundamental::UInt8 => Some(UInt8),
                Fundamental::Int16 => Some(Int16),
                Fundamental::UInt16 => Some(UInt16),
                Fundamental::Int32 => Some(Int32),
                Fundamental::UInt32 => Some(UInt32),
                Fundamental::Int64 => Some(Int64),
                Fundamental::UInt64 => Some(UInt64),
                Fundamental::Char => Some(Char),
                Fundamental::UChar => Some(UChar),
                Fundamental::Short => Some(Short),
                Fundamental::UShort => Some(UShort),
                Fundamental::Int => Some(Int),
                Fundamental::UInt => Some(UInt),
                Fundamental::Long => Some(Long),
                Fundamental::ULong => Some(ULong),
                Fundamental::Size => Some(Size),
                Fundamental::SSize => Some(SSize),
                Fundamental::Float => Some(Float),
                Fundamental::Double => Some(Double),
                Fundamental::Pointer => Some(Void),
                Fundamental::Type => Some(Type),
                Fundamental::Utf8 => Some(Char),
                _ => None,
            }
        }
        else {
            None
        }
    }
}

impl TypeTerminal {
    fn to_rust<'a>(&self, info: &'a Info, env: &Env, ns_id: NsId) -> Cow<'a, str> {
        use self::TypeTerminal::*;
        match *self {
            Void => Cow::from("c_void"),
            Boolean => Cow::from("gboolean"),
            Int8 => Cow::from("i8"),
            UInt8 => Cow::from("u8"),
            Int16 => Cow::from("i16"),
            UInt16 => Cow::from("u16"),
            Int32 => Cow::from("i32"),
            UInt32 => Cow::from("u32"),
            Int64 => Cow::from("i64"),
            UInt64 => Cow::from("u64"),
            Char => Cow::from("c_char"),
            UChar => Cow::from("c_uchar"),
            Short => Cow::from("c_short"),
            UShort => Cow::from("c_ushort"),
            Int => Cow::from("c_int"),
            UInt => Cow::from("c_uint"),
            Long => Cow::from("c_long"),
            ULong => Cow::from("c_ulong"),
            Size => Cow::from("size_t"),
            SSize => Cow::from("ssize_t"),
            Float => Cow::from("c_float"),
            Double => Cow::from("c_double"),
            Type => Cow::from("GType"),
            Id(def_id) => {
                if def_id.ns_id == ns_id {
                    Cow::from(&info.defs[def_id].name[..])
                }
                else {
                    Cow::from(format!("{}::{}", env.namespaces[def_id.ns_id].crate_name,
                        info.defs[def_id].name))
                }
            }
            Postponed(..) => {
                Cow::from("c_void /* error */")
                //unreachable!()
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
pub struct Type(Decorators, TypeTerminal);

#[derive(Debug, Default)]
pub struct Parameter {
    pub name: String,
    pub type_: Type,
}

#[derive(Debug, Default)]
pub struct Field {
    pub name: String,
    pub type_: Type,
    fake: bool,
}

#[derive(Debug)]
pub enum DefKind {
    Alias(Type),
    Bitfield,
    Enumeration,
    Function {
        parameters: Vec<Parameter>,
        ret: Parameter,
    },
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

impl Default for DefKind {
    fn default() -> DefKind {
        DefKind::Alias(Default::default())
    }
}

#[derive(Debug, Default)]
pub struct Def {
    pub name: String,
    gir_tid: Option<library::TypeId>,
    pub kind: DefKind,
    pub ignore: Option<bool>,
    pub public: bool,
    pub transparent: bool,
}

pub struct Info {
    pub defs: NsVec<DefId, Def>,
    gir_tid_index: HashMap<library::TypeId, TypeTerminal>,
    name_index: HashMap<String, TypeTerminal>,
    queue: VecDeque<library::TypeId>,
    rust_type: RefCell<Vec<HashMap<Type, String>>>,
}

struct Env<'a> {
    gir: &'a library::Library,
    namespaces: &'a namespaces::Info,
}

pub fn with_rust_type<R, F>(env: &env::Env, ns_id: NsId, type_: &Type, f: F) -> R
where F: FnOnce(&str) -> R {
    let info = &env.foreign;
    let env = Env {
        gir: &env.library,
        namespaces: &env.namespaces,
    };
    with_rust_type_priv(info, &env, ns_id, type_, f)
}

fn with_rust_type_priv<R, F>(info: &Info, env: &Env, ns_id: NsId, type_: &Type, f: F) -> R
where F: FnOnce(&str) -> R {
    if let Some(s) = info.rust_type.borrow()[ns_id as usize].get(type_) {
        return f(s);
    }

    make_rust_type(info, &env, ns_id, type_);
    f(info.rust_type.borrow()[ns_id as usize].get(type_).unwrap())
}

pub fn run(gir: &library::Library, namespaces: &namespaces::Info) -> Info {
    let mut info = Info {
        defs: NsVec::new(namespaces.len()),
        gir_tid_index: HashMap::new(),
        name_index: HashMap::new(),
        queue: VecDeque::new(),
        rust_type: RefCell::new(vec![HashMap::new(); namespaces.len()]),
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
            Def {
                name: c_identifier.clone(),
                kind: DefKind::Alias(make_type(info, env, typ, target_c_type)),
                ..Default::default()
            }
        }
        Bitfield(library::Bitfield { ref c_type, .. }) => {
            Def {
                name: c_type.clone(),
                kind: DefKind::Bitfield,
                public: true,
                ..Default::default()
            }
        }
        Enumeration(library::Enumeration { ref c_type, .. }) => {
            Def {
                name: c_type.clone(),
                kind: DefKind::Enumeration,
                public: true,
                ..Default::default()
            }
        }
        Function(ref func) => transfer_gir_function(info, env, func),
        Interface(library::Interface { ref c_type, .. }) => {
            Def {
                name: c_type.clone(),
                kind: DefKind::Opaque,
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

fn make_type(info: &mut Info, env: &Env, gir_tid: library::TypeId, c_type_hint: &str) -> Type {
    let gir_type = env.gir.type_(gir_tid);
    let decorators = Decorators::from_c_type(c_type_hint);
    if let Some(type_terminal) = TypeTerminal::primitive(gir_type) {
        Type(decorators, type_terminal)
    }
    else if let Some(&type_terminal) = info.gir_tid_index.get(&gir_tid) {
        Type(decorators, type_terminal)
    }
    else if let library::Type::CArray(tid) = *gir_type {
        let Type(_, type_terminal) = make_type(info, env, tid, "");
        Type(Decorators::mut_ptr(), type_terminal)
    }
    else if let library::Type::FixedArray(tid, length, ref c_type) = *gir_type {
        let Type(mut decs, type_terminal) = make_type(info, env, tid,
            c_type.as_ref().map(|s| &s[..]).unwrap_or(""));
        decs.push_back(&Decorators::fixed_array(length));
        Type(decs, type_terminal)
    }
    else {
        info.queue.push_back(gir_tid);
        Type(decorators, TypeTerminal::Postponed(gir_tid))
    }
}

fn transfer_gir_record(info: &mut Info, env: &Env, ns_id: NsId, record: &library::Record)
        -> Def {
    let name = record.c_type.as_ref().unwrap_or(&record.name).clone();
    let mut def = transfer_gir_recordlike(info, env, ns_id, name, &record.fields, false, record);
    def.public = !record.disguised;
    def
}

fn transfer_gir_class(info: &mut Info, env: &Env, ns_id: NsId, class: &library::Class)
        -> Def {
    let name = class.c_type.clone();
    transfer_gir_recordlike(info, env, ns_id, name, &[], true, class)
}

fn transfer_gir_recordlike(info: &mut Info, env: &Env, ns_id: NsId, name: String,
        record_fields: &[library::Field], is_class: bool, record: &Debug) -> Def {
    fn flush_bits_placeholder(fields: &mut Vec<Field>, bits: u8, count: u8) {
        let bytes = (bits + 7) / 8;
        fields.push(Field {
            name: format!("bits{}", count),
            type_: Type(Decorators::fixed_array(bytes as u16), TypeTerminal::UInt8),
            fake: true,
            ..Default::default()
        });
    }

    let mut fields: Vec<Field> = Vec::new();
    let mut bits: Option<u8> = None;
    let mut bits_placeholder_count = 0u8;
    //let mut ignore = false;

    for field in record_fields {
        let mut field_tid = field.typ;
        if let Some(more_bits) = field.bits {
            match (bits, more_bits, field_tid) {
                (None, 32, tid) if tid == env.gir.find_fundamental(library::Fundamental::Int) => {
                    field_tid = env.gir.find_fundamental(library::Fundamental::Int32);
                }
                (None, 32, tid) if tid == env.gir.find_fundamental(library::Fundamental::UInt) => {
                    field_tid = env.gir.find_fundamental(library::Fundamental::UInt32);
                }
                _ => {
                    bits = Some(bits.unwrap_or(0) + more_bits);
                    continue;
                }
            }
        }
        if let Some(bits) = bits.take() {
            flush_bits_placeholder(&mut fields, bits, bits_placeholder_count);
            bits_placeholder_count += 1;
        }
        match *field {
            library::Field { c_type: Some(ref c_type_hint), .. } => {
                fields.push(Field {
                    name: nameutil::mangle_keywords(&*field.name).into_owned(),
                    type_: make_type(info, env, field_tid, c_type_hint),
                    ..Default::default()
                });
            }
            library::Field { .. } if field_tid.ns_id == namespaces::INTERNAL => {
                match *env.gir.type_(field_tid) {
                    library::Type::Function(ref func) => {
                        let def = transfer_gir_function(info, env, func);
                        let def_id = push_transparent(info, ns_id, def);
                        fields.push(Field {
                            name: nameutil::mangle_keywords(&*field.name).into_owned(),
                            type_: Type(Decorators::option(), TypeTerminal::Id(def_id)),
                            ..Default::default()
                        });
                    }
                    library::Type::Union(ref union) => {
                        let mut def = transfer_gir_union(info, env, ns_id, union);
                        def.name = format!("{}_{}", name, field.name);
                        //def.fake = true;
                        def.gir_tid = Some(field_tid);
                        let def_id = push(info, ns_id, def);
                        fields.push(Field {
                            name: nameutil::mangle_keywords(&*field.name).into_owned(),
                            type_: Type(Decorators::none(), TypeTerminal::Id(def_id)),
                            ..Default::default()
                        });
                    }
                    _ => {
                        warn!("Failed to translate the field `{:?}` from `{:?}`", field, record);
                    }
                }
            }
            library::Field { c_type: None, .. } => {
                // seems harmless
                //warn!("Missing c:type for field `{:?}` from `{:?}`", field, record);
                fields.push(Field {
                    name: nameutil::mangle_keywords(&*field.name).into_owned(),
                    type_: make_type(info, env, field_tid, ""),
                    ..Default::default()
                });
            }
        }
    }

    if let Some(bits) = bits.take() {
        flush_bits_placeholder(&mut fields, bits, bits_placeholder_count);
    }

    Def {
        name: name,
        kind: DefKind::Record {
            fields: fields,
            is_class: is_class,
            fake: false,
        },
        //ignore: ignore,
        ..Default::default()
    }
}

fn transfer_gir_union(info: &mut Info, env: &Env, ns_id: NsId, union: &library::Union) -> Def {
    let mut fields: Vec<Field> = Vec::new();
    let name = union.c_type.as_ref().unwrap_or(&union.name).clone();
    //let mut ignore = false;

    for field in &union.fields {
        match *field {
            library::Field { typ, c_type: Some(ref c_type_hint), .. } => {
                fields.push(Field {
                    name: nameutil::mangle_keywords(&*field.name).into_owned(),
                    type_: make_type(info, env, typ, c_type_hint),
                    ..Default::default()
                });
            }
            library::Field { typ, .. } if typ.ns_id == namespaces::INTERNAL => {
                match *env.gir.type_(typ) {
                    library::Type::Record(ref record) => {
                        let mut def = transfer_gir_record(info, env, ns_id, record);
                        def.name = format!("{}_{}", name, field.name);
                        if let DefKind::Record { ref mut fake, .. } = def.kind {
                            *fake = true;
                        }
                        def.gir_tid = Some(typ);
                        let def_id = push(info, ns_id, def);
                        fields.push(Field {
                            name: nameutil::mangle_keywords(&*field.name).into_owned(),
                            type_: Type(Decorators::none(), TypeTerminal::Id(def_id)),
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

    Def {
        name: name,
        kind: DefKind::Union {
            fields: fields,
        },
        //ignore: ignore,
        ..Default::default()
    }
}

fn transfer_gir_function(info: &mut Info, env: &Env, function: &library::Function) -> Def {
    let mut params: Vec<Parameter> = vec![];
    for param in &function.parameters {
        params.push(Parameter {
            name: nameutil::mangle_keywords(&*param.name).into_owned(),
            type_: make_type(info, env, param.typ, &param.c_type),
            ..Default::default()
        });
    }
    let ret = Parameter {
        type_: make_type(info, env, function.ret.typ, &function.ret.c_type),
        ..Default::default()
    };
    let name = function.c_identifier.as_ref().unwrap_or(&function.name);
    Def {
        name: nameutil::mangle_keywords(&name[..]).into_owned(),
        kind: DefKind::Function {
            parameters: params,
            ret: ret,
        },
        ..Default::default()
    }
}

fn push(info: &mut Info, ns_id: NsId, def: Def) -> DefId {
    trace!("Adding `{:?}`", def);
    let gir_tid = def.gir_tid;
    let name = def.name.clone();
    let def_id = info.defs.push(ns_id, def);
    if let Some(gir_tid) = gir_tid {
        info.gir_tid_index.insert(gir_tid, TypeTerminal::Id(def_id));
    }
    info.name_index.insert(name, TypeTerminal::Id(def_id));
    def_id
}

fn push_transparent(info: &mut Info, ns_id: NsId, mut def: Def) -> DefId {
    def.transparent = true;
    trace!("Adding `{:?}`", def);
    let def_id = info.defs.push(ns_id, def);
    def_id
}

fn resolve_postponed_types(info: &mut Info, env: &Env) {
    for ns_id in 0..env.namespaces.len() as NsId {
        for def_id in info.defs.ids_by_ns(ns_id) {
            let Def { ref mut kind, ref mut ignore, .. } = info.defs[def_id];
            match *kind {
                DefKind::Alias(ref mut type_) => {
                    resolve(&info.gir_tid_index, env, type_, ignore);
                }
                DefKind::Function { ref mut parameters, ref mut ret, .. } => {
                    for param in parameters.iter_mut() {
                        resolve(&info.gir_tid_index, env, &mut param.type_, ignore);
                    }
                    resolve(&info.gir_tid_index, env, &mut ret.type_, ignore);
                }
                DefKind::Record { ref mut fields, .. } => {
                    for field in fields.iter_mut() {
                        resolve(&info.gir_tid_index, env, &mut field.type_, ignore);
                    }
                }
                _ => {}
            }
        }
    }
}

fn resolve(gir_tid_index: &HashMap<library::TypeId, TypeTerminal>, env: &Env,
        type_: &mut Type, ignore: &mut Option<bool>) {
    let Type(_, ref mut type_terminal) = *type_;
    if let TypeTerminal::Postponed(gir_tid) = *type_terminal {
        if let Some(&resolved) = gir_tid_index.get(&gir_tid) {
            trace!("Resolved `{:?}` to `{:?}`", gir_tid, resolved);
            *type_terminal = resolved;
        }
        else {
            info!("Couldn't resolve `{:?}`", env.gir.type_(gir_tid));
            *ignore = Some(true);
        }
    }
}

fn make_rust_type(info: &Info, env: &Env, ns_id: NsId, type_: &Type) {
    let Type(ref decorators, ref type_terminal) = *type_;
    if let TypeTerminal::Id(def_id) = *type_terminal {
        let def = &info.defs[def_id];
        if def.transparent {
            let type_str = match def.kind {
                DefKind::Alias(ref target_type) => {
                    with_rust_type_priv(info, env, ns_id, target_type, |s| decorators.to_rust(s))
                }
                DefKind::Function { ref parameters, ref ret, .. } => {
                    let param_str = parameters.iter()
                        .map(|param| {
                            with_rust_type_priv(info, env, ns_id, &param.type_, |s| String::from(s))
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let ret_str = match ret.type_ {
                        Type(ref decs, TypeTerminal::Void) if decs.is_none() => None,
                        ref t => {
                            Some(with_rust_type_priv(info, env, ns_id, t, |s| format!(" -> {}", s)))
                        }
                    };
                    decorators.to_rust(&format!("fn({}){}", param_str,
                        ret_str.as_ref().map(|s| &s[..]).unwrap_or("")))
                }
                _ => unreachable!(),
            };
            info.rust_type.borrow_mut()[ns_id as usize].insert(type_.clone(), type_str);
            return;
        }
    }
    let type_str = decorators.to_rust(&type_terminal.to_rust(info, env, ns_id));
    info.rust_type.borrow_mut()[ns_id as usize].insert(type_.clone(), type_str);
}

fn fix_weird_types(info: &mut Info) {
    fn atomize(info: &mut Info, name: &str) {
        if let Some(&TypeTerminal::Id(def_id)) = info.name_index.get(name) {
            let mut def = Def {
                name: String::from(name),
                ..Default::default()
            };
            mem::swap(&mut def, &mut info.defs[def_id]);
            def.name = format!("_{}", name);
            let new_def_id = push(info, def_id.ns_id, def);
            info.defs[def_id].kind =
                DefKind::Alias(Type(Decorators::mut_ptr(), TypeTerminal::Id(new_def_id)));
        }
    }

    atomize(info, "GIConv");
}
