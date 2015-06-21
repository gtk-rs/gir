use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::rc::Rc;

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

#[derive(Debug)]
pub enum Fundamental {
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
    None,
    Unsupported,
}

impl Fundamental {
    pub fn by_name(name: &str) -> Option<Fundamental> {
        use self::Fundamental::*;
        match name {
            "gboolean" => Some(Boolean),
            "gint8" => Some(Int8),
            "guint8" => Some(UInt8),
            "gint16" => Some(Int16),
            "guint16" => Some(UInt16),
            "gint32" => Some(Int32),
            "guint32" => Some(UInt32),
            "gint64" => Some(Int64),
            "guint64" => Some(UInt64),
            "gchar" => Some(Char),
            "guchar" => Some(UChar),
            "gint" => Some(Int),
            "guint" => Some(UInt),
            "glong" => Some(Long),
            "gulong" => Some(ULong),
            "gsize" => Some(Size),
            "gssize" => Some(SSize),
            "gfloat" => Some(Float),
            "gdouble" => Some(Double),
            "long double" => Some(Unsupported),
            "gunichar" => Some(UniChar),
            "gpointer" => Some(Pointer),
            "va_list" => Some(Unsupported),
            "varargs" => Some(VarArgs),
            "utf8" => Some(Utf8),
            "filename" => Some(Filename),
            "GType" => Some(Type),
            "none" => Some(None),
            _ => Option::None,
        }
    }
}

pub struct Alias {
    pub name: String,
    pub c_identifier: String,
    pub typ: TypeRef,
}

pub struct Constant {
    pub name: String,
    pub typ: TypeRef,
    pub value: String,
}

pub struct Member {
    pub name: String,
    pub c_identifier: String,
    pub value: String,
}

pub struct Enumeration {
    pub name: String,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
}

pub struct Bitfield {
    pub name: String,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
}

pub struct Record {
    pub name: String,
    pub functions: Vec<Function>,
}

pub struct Field {
    pub name: String,
    pub typ: TypeRef,
}

pub struct Union {
    pub name: String,
    pub fields: Vec<Field>,
    pub functions: Vec<Function>,
}

pub struct Parameter {
    pub name: String,
    pub typ: TypeRef,
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
    pub functions: Vec<Function>,
}

pub struct Class {
    pub name: String,
    pub functions: Vec<Function>,
}

pub type TypeRef = Rc<RefCell<Type>>;

pub enum Type {
    Unresolved,
    Fundamental(Fundamental),
    Alias(Alias),
    Enumeration(Enumeration),
    Bitfield(Bitfield),
    Record(Record),
    Union(Union),
    Callback(Function),
    Interface(Interface),
    Class(Class),
    Array(TypeRef),
    HashTable(TypeRef, TypeRef),
    List(TypeRef),
    SList(TypeRef),
}

impl Type {
    pub fn new(typ: Type) -> TypeRef {
        Rc::new(RefCell::new(typ))
    }

    pub fn container(name: &str, mut inner: Vec<TypeRef>) -> Option<TypeRef> {
        match (name, inner.len()) {
            ("array", 1) => Some(Type::new(Type::Array(inner.remove(0)))),
            ("GLib.HashTable", 2) => Some(Type::new(
                                    Type::HashTable(inner.remove(0), inner.remove(0)))),
            ("GLib.List", 1) => Some(Type::new(Type::List(inner.remove(0)))),
            ("GLib.SList", 1) => Some(Type::new(Type::SList(inner.remove(0)))),
            _ => None,
        }
    }
}

pub trait AsArg {
    fn as_arg(&self) -> String;
}

impl AsArg for Fundamental {
    fn as_arg(&self) -> String {
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
        }.to_string()
    }
}

impl AsArg for Type {
    fn as_arg(&self) -> String {
        use self::Type::*;
        match *self {
            Unresolved => panic!("Unresolved type"),
            Fundamental(ref x) => x.as_arg(),
            Alias(ref x) => x.typ.borrow().as_arg(),
            Enumeration(ref x) => x.name.clone(),
            Bitfield(ref x) => x.name.clone(),
            Record(ref x) => format!("*mut {}", &x.name),
            Union(ref x) => format!("*mut {}", &x.name),
            Callback(_) => "TODO".to_string(),
            Interface(ref x) => format!("*mut {}", &x.name),
            Class(ref x) => format!("*mut {}", &x.name),
            Array(ref x) => format!("*mut {}", x.borrow().as_arg()),
            HashTable(_, _)  => "*mut GHashTable".to_string(),
            List(_)  => "*mut GList".to_string(),
            SList(_)  => "*mut GSList".to_string(),
        }
    }
}

pub struct Library {
    pub types: BTreeMap<String, TypeRef>,
    pub constants: BTreeMap<String, Constant>,
    pub functions: BTreeMap<String, Function>,
    pub namespaces: HashSet<String>,
}

impl Library {
    pub fn new() -> Library {
        Library { types: BTreeMap::new(), namespaces: HashSet::new(),
                  constants: BTreeMap::new(), functions: BTreeMap:: new() }
    }

    pub fn get_type(&mut self, namespace: &str, name: &str) -> TypeRef {
        if let Some(typ) = self.types.get(name) {
            return typ.clone();
        }

        if name.contains('.') {
            let name = name.to_string();
            let typ = Type::new(Type::Unresolved);
            self.types.insert(name, typ.clone());
            return typ;
        }
        else {
            if let Some(typ) = Fundamental::by_name(name) {
                let name = name.to_string();
                let typ = Type::new(Type::Fundamental(typ));
                self.types.insert(name.clone(), typ.clone());
                return typ;
            }

            let name = format!("{}.{}", namespace, name);

            if let Some(typ) = self.types.get(&name) {
                return typ.clone();
            }

            let typ = Type::new(Type::Unresolved);
            self.types.insert(name, typ.clone());
            typ
        }
    }

    pub fn forget_type(&mut self, namespace: &str, name: &str) {
        if name.contains('.') {
            self.types.remove(name);
        }
        else {
            let name = format!("{}.{}", namespace, name);
            self.types.remove(&name);
        }
    }

    pub fn check_resolved(&self) {
        let mut err = Vec::new();
        for (ref name, ref typ) in &self.types {
            if let Type::Unresolved = *typ.borrow() {
                err.push(*name);
            }
        }
        if !err.is_empty() {
            panic!("Incomplete library, unresolved: {:?}", err);
        }
    }


}

