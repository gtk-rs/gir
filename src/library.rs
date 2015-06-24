use std::collections::HashMap;

pub enum Transfer {
    None,
    Container,
    Full,
}

impl Transfer {
    pub fn by_name(name: &str) -> Option<Transfer> {
        use self::Transfer::*;
        match name {
            "none" => Some(None),
            "container" => Some(Container),
            "full" => Some(Full),
            _ => Option::None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fundamental {
    None,
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
    Int,
    UInt,
    Long,
    ULong,
    Size,
    SSize,
    Float,
    Double,
    Pointer,
    VarArgs,
    UniChar,
    Utf8,
    Filename,
    Type,
    Unsupported,
}

pub const FUNDAMENTAL: [(&'static str, Fundamental); 28] = [
    ("none", Fundamental::None),
    ("gboolean", Fundamental::Boolean),
    ("gint8", Fundamental::Int8),
    ("guint8", Fundamental::UInt8),
    ("gint16", Fundamental::Int16),
    ("guint16", Fundamental::UInt16),
    ("gint32", Fundamental::Int32),
    ("guint32", Fundamental::UInt32),
    ("gint64", Fundamental::Int64),
    ("guint64", Fundamental::UInt64),
    ("gchar", Fundamental::Char),
    ("guchar", Fundamental::UChar),
    ("gint", Fundamental::Int),
    ("guint", Fundamental::UInt),
    ("glong", Fundamental::Long),
    ("gulong", Fundamental::ULong),
    ("gsize", Fundamental::Size),
    ("gssize", Fundamental::SSize),
    ("gfloat", Fundamental::Float),
    ("gdouble", Fundamental::Double),
    ("long double", Fundamental::Unsupported),
    ("gunichar", Fundamental::UniChar),
    ("gpointer", Fundamental::Pointer),
    ("va_list", Fundamental::Unsupported),
    ("varargs", Fundamental::VarArgs),
    ("utf8", Fundamental::Utf8),
    ("filename", Fundamental::Filename),
    ("GType", Fundamental::Type),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeId {
    ns_id: u16,
    id: u32,
}

pub struct Alias {
    pub name: String,
    pub c_identifier: String,
    pub typ: TypeId,
}

pub struct Constant {
    pub name: String,
    pub typ: TypeId,
    pub value: String,
}

pub struct Member {
    pub name: String,
    pub c_identifier: String,
    pub value: String,
}

pub struct Enumeration {
    pub name: String,
    pub glib_name: String,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
}

pub struct Bitfield {
    pub name: String,
    pub glib_name: String,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
}

pub struct Record {
    pub name: String,
    pub glib_name: String,
    pub functions: Vec<Function>,
}

pub struct Field {
    pub name: String,
    pub typ: TypeId,
}

pub struct Union {
    pub name: String,
    pub glib_name: String,
    pub fields: Vec<Field>,
    pub functions: Vec<Function>,
}

pub struct Parameter {
    pub name: String,
    pub typ: TypeId,
    pub transfer: Transfer,
}

pub struct Function {
    pub name: String,
    pub c_identifier: String,
    pub parameters: Vec<Parameter>,
    pub ret: Parameter,
}

pub struct Interface {
    pub name: String,
    pub glib_name: String,
    pub functions: Vec<Function>,
}

pub struct Class {
    pub name: String,
    pub glib_name: String,
    pub functions: Vec<Function>,
    pub parent: Option<TypeId>,
    pub implements: Vec<TypeId>,
}

pub enum Type {
    Fundamental(Fundamental),
    Alias(Alias),
    Enumeration(Enumeration),
    Bitfield(Bitfield),
    Record(Record),
    Union(Union),
    Callback(Function),
    Interface(Interface),
    Class(Class),
    Array(TypeId),
    HashTable(TypeId, TypeId),
    List(TypeId),
    SList(TypeId),
}

impl Type {
    pub fn as_class(&self) -> Option<&Class> {
        if let &Type::Class(ref x) = self { Some(x) } else { None }
    }

    pub fn container(library: &mut Library, name: &str, mut inner: Vec<TypeId>) -> Option<TypeId> {
        match (name, inner.len()) {
            ("array", 1) => {
                let tid = inner.remove(0);
                Some((format!("array(#{:?})", tid), Type::Array(tid)))
            }
            ("GLib.HashTable", 2) => {
                let k_tid = inner.remove(0);
                let v_tid = inner.remove(0);
                Some((format!("HashTable(#{:?}, #{:?})", k_tid, v_tid), Type::HashTable(k_tid, v_tid)))
            }
            ("GLib.List", 1) => {
                let tid = inner.remove(0);
                Some((format!("List(#{:?})", tid), Type::List(tid)))
            }
            ("GLib.SList", 1) => {
                let tid = inner.remove(0);
                Some((format!("SList(#{:?})", tid), Type::SList(tid)))
            }
            _ => None,
        }.map(|(name, typ)| library.add_type(INTERNAL_NAMESPACE, &name, typ))
    }
}

pub trait AsArg {
    fn as_arg(&self, library: &Library) -> String;
}

impl AsArg for Fundamental {
    fn as_arg(&self, _: &Library) -> String {
        use self::Fundamental::*;
        match *self {
            Boolean => "gboolean",
            Int8 => "gint8",
            UInt8 => "guint8",
            Int16 => "gint16",
            UInt16 => "guint16",
            Int32 => "gint32",
            UInt32 => "guint32",
            Int64 => "gint64",
            UInt64 => "guint64",
            Char => "gchar",
            UChar => "guchar",
            Int => "gint",
            UInt => "guint",
            Long => "glong",
            ULong => "gulong",
            Size => "gsize",
            SSize => "gssize",
            Float => "gfloat",
            Double => "gdouble",
            UniChar => "gunichar",
            Pointer => "gpointer",
            VarArgs => "...",
            Utf8 => "*const c_char",
            Filename => "*const c_char",
            Type => "GType",
            None => "c_void",
            Unsupported => panic!("unsupported type"),
        }.into()
    }
}

impl AsArg for Type {
    fn as_arg(&self, library: &Library) -> String {
        use self::Type::*;
        match *self {
            Fundamental(ref x) => x.as_arg(library),
            Alias(ref x) => library.type_(x.typ).as_arg(library),
            Enumeration(ref x) => x.name.clone(),
            Bitfield(ref x) => x.name.clone(),
            Record(ref x) => format!("*mut {}", &x.name),
            Union(ref x) => format!("*mut {}", &x.name),
            Callback(_) => "TODO".into(),
            Interface(ref x) => format!("*mut {}", &x.name),
            Class(ref x) => format!("*mut {}", &x.name),
            Array(x) => format!("*mut {}", library.type_(x).as_arg(library)),
            HashTable(_, _)  => "*mut GHashTable".into(),
            List(_)  => "*mut GList".into(),
            SList(_)  => "*mut GSList".into(),
        }
    }
}

pub struct Namespace {
    pub name: String,
    pub types: Vec<Option<Type>>,
    pub index: HashMap<String, u32>,
    pub constants: Vec<Constant>,
    pub functions: Vec<Function>,
}

impl Namespace {
    fn new(name: &str) -> Namespace {
        Namespace {
            name: name.into(),
            types: Vec::new(),
            index: HashMap::new(),
            constants: Vec::new(),
            functions: Vec::new(),
        }
    }

    fn add_constant(&mut self, c: Constant) {
        self.constants.push(c);
    }

    fn add_function(&mut self, f: Function) {
        self.functions.push(f);
    }

    fn type_(&self, id: u32) -> &Type {
        self.types[id as usize].as_ref().unwrap()
    }

    fn add_type(&mut self, name: &str, typ: Option<Type>) -> u32 {
        if let Some(id) = self.find_type(name) {
            self.types[id as usize] = typ;
            id
        }
        else {
            let id = self.types.len() as u32;
            self.types.push(typ);
            self.index.insert(name.into(), id);
            id
        }
    }

    fn find_type(&self, name: &str) -> Option<u32> {
        self.index.get(name).cloned()
    }

    fn unresolved(&self) -> Vec<&str> {
        self.index.iter().filter_map(|(name, &id)| {
            if self.types[id as usize].is_none() {
                Some(&name[..])
            } else {
                None
            }
        }).collect()
    }
}

pub const INTERNAL_NAMESPACE_NAME: &'static str = "*";
pub const INTERNAL_NAMESPACE: u16 = 0;

pub struct Library {
    pub namespaces: Vec<Namespace>,
    pub index: HashMap<String, u16>,
}

impl Library {
    pub fn new() -> Library {
        let mut library = Library {
            namespaces: Vec::new(),
            index: HashMap::new(),
        };
        assert!(library.add_namespace(INTERNAL_NAMESPACE_NAME) == INTERNAL_NAMESPACE);
        library.namespace_mut(INTERNAL_NAMESPACE).name = INTERNAL_NAMESPACE_NAME.into();
        for &(name, t) in &FUNDAMENTAL {
            library.add_type(INTERNAL_NAMESPACE, name, Type::Fundamental(t));
        }
        library
    }

    pub fn namespace(&self, ns_id: u16) -> &Namespace {
        &self.namespaces[ns_id as usize]
    }

    pub fn namespace_mut(&mut self, ns_id: u16) -> &mut Namespace {
        &mut self.namespaces[ns_id as usize]
    }

    pub fn find_namespace(&self, name: &str) -> Option<u16> {
        self.index.get(name).cloned()
    }

    pub fn add_namespace(&mut self, name: &str) -> u16 {
        if let Some(&id) = self.index.get(name) {
            id
        }
        else {
            let id = self.namespaces.len() as u16;
            self.namespaces.push(Namespace::new(name));
            self.index.insert(name.into(), id);
            id
        }
    }

    pub fn add_constant(&mut self, ns_id: u16, c: Constant) {
        self.namespace_mut(ns_id).add_constant(c);
    }

    pub fn add_function(&mut self, ns_id: u16, f: Function) {
        self.namespace_mut(ns_id).add_function(f);
    }

    pub fn add_type(&mut self, ns_id: u16, name: &str, typ: Type) -> TypeId {
        TypeId { ns_id: ns_id, id: self.namespace_mut(ns_id).add_type(name, Some(typ)) }
    }

    pub fn find_type(&self, current_ns_id: u16, name: &str) -> Option<TypeId> {
        let (ns, name) = split_ns_name(name);

        if let Some(ns) = ns {
            self.find_namespace(ns).and_then(|ns_id| {
                self.namespace(ns_id).find_type(name).map(|id| TypeId { ns_id: ns_id, id: id })
            })
        }
        else if let Some(id) = self.namespace(current_ns_id).find_type(name) {
            Some(TypeId { ns_id: current_ns_id, id: id })
        }
        else if let Some(id) = self.namespace(INTERNAL_NAMESPACE).find_type(name) {
            Some(TypeId { ns_id: INTERNAL_NAMESPACE, id: id })
        }
        else {
            None
        }
    }

    pub fn find_or_stub_type(&mut self, current_ns_id: u16, name: &str) -> TypeId {
        if let Some(tid) = self.find_type(current_ns_id, name) {
            return tid;
        }

        let (ns, name) = split_ns_name(name);

        if let Some(ns) = ns {
            let ns_id = self.find_namespace(ns).unwrap_or_else(|| self.add_namespace(ns));
            let ns = self.namespace_mut(ns_id);
            let id = ns.find_type(name).unwrap_or_else(|| ns.add_type(name, None));
            return TypeId { ns_id: ns_id, id: id };
        }

        let id = self.namespace_mut(current_ns_id).add_type(name, None);
        TypeId { ns_id: current_ns_id, id: id }
    }

    pub fn type_(&self, tid: TypeId) -> &Type {
        self.namespace(tid.ns_id).type_(tid.id)
    }

    pub fn check_resolved(&self) {
        let list: Vec<_> = self.index.iter().flat_map(|(name, &id)| {
            let name = name.clone();
            self.namespace(id).unresolved().into_iter().map(move |s| format!("{}.{}", name, s))
        }).collect();

        if !list.is_empty() {
            panic!("Incomplete library, unresolved: {:?}", list);
        }
    }
}

fn split_ns_name(name: &str) -> (Option<&str>, &str) {
    let mut parts = name.split('.');
    let name = parts.next_back().unwrap();
    let ns = parts.next_back();
    assert!(ns.is_none() || parts.next().is_none());
    (ns, name)
}
