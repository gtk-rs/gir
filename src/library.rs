use crate::{
    analysis::conversion_type::ConversionType, env::Env, nameutil::split_namespace_name, traits::*,
    version::Version,
};
use std::{
    cmp::{Ord, Ordering, PartialOrd},
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
    iter::Iterator,
    ops::{Deref, DerefMut},
    str::FromStr,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transfer {
    None,
    Container,
    Full,
}

impl FromStr for Transfer {
    type Err = String;
    fn from_str(name: &str) -> Result<Transfer, String> {
        use self::Transfer::*;
        match name {
            "none" => Ok(None),
            "container" => Ok(Container),
            "full" => Ok(Full),
            _ => Err("Unknown ownership transfer mode".into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterDirection {
    In,
    Out,
    InOut,
    Return,
}

impl ParameterDirection {
    pub fn is_out(self) -> bool {
        self == ParameterDirection::Out || self == ParameterDirection::InOut
    }
}

impl FromStr for ParameterDirection {
    type Err = String;
    fn from_str(name: &str) -> Result<ParameterDirection, String> {
        use self::ParameterDirection::*;
        match name {
            "in" => Ok(In),
            "out" => Ok(Out),
            "inout" => Ok(InOut),
            _ => Err("Unknown parameter direction".into()),
        }
    }
}

impl Default for ParameterDirection {
    fn default() -> ParameterDirection {
        ParameterDirection::In
    }
}

/// Annotation describing lifetime requirements / guarantees of callback parameters,
/// that is callback itself and associated user data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterScope {
    /// Parameter is not of callback type.
    None,
    /// Used only for the duration of the call.
    ///
    /// Can be invoked multiple times.
    Call,
    /// Used for the duration of the asynchronous call.
    ///
    /// Invoked exactly once when asynchronous call completes.
    Async,
    /// Used until notified with associated destroy notify parameter.
    ///
    /// Can be invoked multiple times.
    Notified,
}

impl ParameterScope {
    pub fn is_call(self) -> bool {
        match self {
            ParameterScope::Call => true,
            _ => false,
        }
    }

    pub fn is_async(self) -> bool {
        match self {
            ParameterScope::Async => true,
            _ => false,
        }
    }
}

impl Default for ParameterScope {
    fn default() -> Self {
        ParameterScope::None
    }
}

impl FromStr for ParameterScope {
    type Err = String;

    fn from_str(name: &str) -> Result<ParameterScope, String> {
        match name {
            "call" => Ok(ParameterScope::Call),
            "async" => Ok(ParameterScope::Async),
            "notified" => Ok(ParameterScope::Notified),
            _ => Err(format!("Unknown parameter scope type: {}", name)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Nullable(pub bool);

impl Deref for Nullable {
    type Target = bool;
    fn deref(&self) -> &bool {
        &self.0
    }
}

impl DerefMut for Nullable {
    fn deref_mut(&mut self) -> &mut bool {
        &mut self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionKind {
    Constructor,
    Function,
    Method,
    Global,
}

impl FromStr for FunctionKind {
    type Err = String;
    fn from_str(name: &str) -> Result<FunctionKind, String> {
        use self::FunctionKind::*;
        match name {
            "constructor" => Ok(Constructor),
            "function" => Ok(Function),
            "method" => Ok(Method),
            "callback" => Ok(Function),
            "global" => Ok(Global),
            _ => Err("Unknown function kind".into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Concurrency {
    None,
    SendUnique,
    Send,
    SendSync,
}

impl FromStr for Concurrency {
    type Err = String;
    fn from_str(name: &str) -> Result<Concurrency, String> {
        use self::Concurrency::*;
        match name {
            "none" => Ok(None),
            "send-unique" => Ok(SendUnique),
            "send" => Ok(Send),
            "send+sync" => Ok(SendSync),
            _ => Err("Unknown concurrency kind".into()),
        }
    }
}

impl Default for Concurrency {
    fn default() -> Concurrency {
        Concurrency::None
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
    Pointer,
    VarArgs,
    UniChar,
    Utf8,
    Filename,
    Type,
    IntPtr,
    UIntPtr,
    //Same encoding as Filename but can contains any string
    //Not defined in GLib directly
    OsString,
    Unsupported,
}

impl Fundamental {
    pub fn requires_conversion(self) -> bool {
        match self {
            Fundamental::Int8
            | Fundamental::UInt8
            | Fundamental::Int16
            | Fundamental::UInt16
            | Fundamental::Int32
            | Fundamental::UInt32
            | Fundamental::Int64
            | Fundamental::UInt64
            | Fundamental::Char
            | Fundamental::UChar
            | Fundamental::Short
            | Fundamental::UShort
            | Fundamental::Int
            | Fundamental::UInt
            | Fundamental::Long
            | Fundamental::ULong
            | Fundamental::Size
            | Fundamental::SSize
            | Fundamental::Float
            | Fundamental::Double => false,
            _ => true,
        }
    }
}

const FUNDAMENTAL: &[(&str, Fundamental)] = &[
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
    ("gshort", Fundamental::Short),
    ("gushort", Fundamental::UShort),
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
    ("gconstpointer", Fundamental::Pointer),
    ("gpointer", Fundamental::Pointer),
    ("va_list", Fundamental::Unsupported),
    ("varargs", Fundamental::VarArgs),
    ("utf8", Fundamental::Utf8),
    ("filename", Fundamental::Filename),
    ("GType", Fundamental::Type),
    ("gintptr", Fundamental::IntPtr),
    ("guintptr", Fundamental::UIntPtr),
    //TODO: this is temporary name, change it when type added to GLib
    ("os_string", Fundamental::OsString),
];

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypeId {
    pub ns_id: u16,
    pub id: u32,
}

impl TypeId {
    pub fn full_name(self, library: &Library) -> String {
        let ns_name = &library.namespace(self.ns_id).name;
        let type_ = &library.type_(self);
        format!("{}.{}", ns_name, &type_.get_name())
    }

    pub fn tid_none() -> TypeId {
        Default::default()
    }

    pub fn tid_bool() -> TypeId {
        TypeId { ns_id: 0, id: 1 }
    }

    pub fn tid_utf8() -> TypeId {
        TypeId { ns_id: 0, id: 28 }
    }

    pub fn tid_filename() -> TypeId {
        TypeId { ns_id: 0, id: 29 }
    }

    pub fn tid_os_string() -> TypeId {
        TypeId { ns_id: 0, id: 33 }
    }

    pub fn is_fundamental_type(self, env: &Env) -> bool {
        env.library.type_(self).is_fundamental_type(env)
    }
}

#[derive(Debug)]
pub struct Alias {
    pub name: String,
    pub c_identifier: String,
    pub typ: TypeId,
    pub target_c_type: String,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
}

#[derive(Debug)]
pub struct Constant {
    pub name: String,
    pub c_identifier: String,
    pub typ: TypeId,
    pub c_type: String,
    pub value: String,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
}

#[derive(Debug)]
pub struct Member {
    pub name: String,
    pub c_identifier: String,
    pub value: String,
    pub doc: Option<String>,
}

#[derive(Debug)]
pub struct Enumeration {
    pub name: String,
    pub c_type: String,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
    pub error_domain: Option<String>,
    pub glib_get_type: Option<String>,
}

#[derive(Debug)]
pub struct Bitfield {
    pub name: String,
    pub c_type: String,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
    pub glib_get_type: Option<String>,
}

#[derive(Default, Debug)]
pub struct Record {
    pub name: String,
    pub c_type: String,
    pub glib_get_type: Option<String>,
    pub gtype_struct_for: Option<String>,
    pub fields: Vec<Field>,
    pub functions: Vec<Function>,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
    /// A 'disguised' record is one where the c:type is a typedef that
    /// doesn't look like a pointer, but is internally: typedef struct _X *X;
    pub disguised: bool,
}

#[derive(Default, Debug)]
pub struct Field {
    pub name: String,
    pub typ: TypeId,
    pub c_type: Option<String>,
    pub private: bool,
    pub bits: Option<u8>,
    pub array_length: Option<u32>,
    pub doc: Option<String>,
}

#[derive(Default, Debug)]
pub struct Union {
    pub name: String,
    pub c_type: Option<String>,
    pub glib_get_type: Option<String>,
    pub fields: Vec<Field>,
    pub functions: Vec<Function>,
    pub doc: Option<String>,
}

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub readable: bool,
    pub writable: bool,
    pub construct: bool,
    pub construct_only: bool,
    pub typ: TypeId,
    pub c_type: Option<String>,
    pub transfer: Transfer,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Parameter {
    pub name: String,
    pub typ: TypeId,
    pub c_type: String,
    pub instance_parameter: bool,
    pub direction: ParameterDirection,
    pub transfer: Transfer,
    pub caller_allocates: bool,
    pub nullable: Nullable,
    pub allow_none: bool,
    pub array_length: Option<u32>,
    pub is_error: bool,
    pub doc: Option<String>,
    pub scope: ParameterScope,
    /// Index of the user data parameter associated with the callback.
    pub closure: Option<usize>,
    /// Index of the destroy notification parameter associated with the callback.
    pub destroy: Option<usize>,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub c_identifier: Option<String>,
    pub kind: FunctionKind,
    pub parameters: Vec<Parameter>,
    pub ret: Parameter,
    pub throws: bool,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
}

#[derive(Debug)]
pub struct Signal {
    pub name: String,
    pub parameters: Vec<Parameter>,
    pub ret: Parameter,
    pub is_action: bool,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
}

#[derive(Default, Debug)]
pub struct Interface {
    pub name: String,
    pub c_type: String,
    pub type_struct: Option<String>,
    pub c_class_type: Option<String>,
    pub glib_get_type: String,
    pub functions: Vec<Function>,
    pub signals: Vec<Signal>,
    pub properties: Vec<Property>,
    pub prerequisites: Vec<TypeId>,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
}

#[derive(Default, Debug)]
pub struct Class {
    pub name: String,
    pub c_type: String,
    pub type_struct: Option<String>,
    pub c_class_type: Option<String>,
    pub glib_get_type: String,
    pub fields: Vec<Field>,
    pub functions: Vec<Function>,
    pub signals: Vec<Signal>,
    pub properties: Vec<Property>,
    pub parent: Option<TypeId>,
    pub implements: Vec<TypeId>,
    pub final_type: bool,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
}

#[derive(Debug)]
pub struct Custom {
    pub name: String,
    pub conversion_type: ConversionType,
}

macro_rules! impl_lexical_ord {
    () => ();
    ($name:ident => $field:ident, $($more_name:ident => $more_field:ident,)*) => (
        impl_lexical_ord!($($more_name => $more_field,)*);

        impl PartialEq for $name {
            fn eq(&self, other: &$name) -> bool {
                self.$field.eq(&other.$field)
            }
        }

        impl Eq for $name { }

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &$name) -> Option<Ordering> {
                self.$field.partial_cmp(&other.$field)
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &$name) -> Ordering {
                self.$field.cmp(&other.$field)
            }
        }
    );
}

impl_lexical_ord!(
    Alias => c_identifier,
    Bitfield => c_type,
    Class => c_type,
    Enumeration => c_type,
    Function => c_identifier,
    Interface => c_type,
    Record => c_type,
    Union => c_type,
    Custom => name,
);

#[allow(clippy::large_enum_variant)]
#[derive(Debug, PartialEq)]
pub enum Type {
    Fundamental(Fundamental),
    Alias(Alias),
    Enumeration(Enumeration),
    Bitfield(Bitfield),
    Record(Record),
    Union(Union),
    Function(Function),
    Interface(Interface),
    Class(Class),
    Custom(Custom),
    Array(TypeId),
    CArray(TypeId),
    FixedArray(TypeId, u16, Option<String>),
    PtrArray(TypeId),
    HashTable(TypeId, TypeId),
    List(TypeId),
    SList(TypeId),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                Type::Fundamental(_) => "Fundamental",
                Type::Alias(_) => "Alias",
                Type::Enumeration(_) => "Enumeration",
                Type::Bitfield(_) => "Bitfield",
                Type::Record(_) => "Record",
                Type::Union(_) => "Union",
                Type::Function(_) => "Function",
                Type::Interface(_) => "Interface",
                Type::Class(_) => "Class",
                Type::Custom(_) => "Custom",
                Type::Array(_) => "Array",
                Type::CArray(_) => "CArray",
                Type::FixedArray(_, _, _) => "FixedArray",
                Type::PtrArray(_) => "PtrArray",
                Type::HashTable(_, _) => "HashTable",
                Type::List(_) => "List",
                Type::SList(_) => "SList",
            }
        )
    }
}

impl Type {
    pub fn get_name(&self) -> String {
        use self::Type::*;
        match *self {
            Fundamental(fund) => format!("{:?}", fund),
            Alias(ref alias) => alias.name.clone(),
            Enumeration(ref enum_) => enum_.name.clone(),
            Bitfield(ref bit_field) => bit_field.name.clone(),
            Record(ref rec) => rec.name.clone(),
            Union(ref union) => union.name.clone(),
            Function(ref func) => func.name.clone(),
            Interface(ref interface) => interface.name.clone(),
            Array(type_id) => format!("Array {:?}", type_id),
            Class(ref class) => class.name.clone(),
            Custom(ref custom) => custom.name.clone(),
            CArray(type_id) => format!("CArray {:?}", type_id),
            FixedArray(type_id, size, _) => format!("FixedArray {:?}; {}", type_id, size),
            PtrArray(type_id) => format!("PtrArray {:?}", type_id),
            HashTable(key_type_id, value_type_id) => {
                format!("HashTable {:?}/{:?}", key_type_id, value_type_id)
            }
            List(type_id) => format!("List {:?}", type_id),
            SList(type_id) => format!("SList {:?}", type_id),
        }
    }

    pub fn get_deprecated_version(&self) -> Option<Version> {
        use self::Type::*;
        match *self {
            Fundamental(_) => None,
            Alias(_) => None,
            Enumeration(ref enum_) => enum_.deprecated_version,
            Bitfield(ref bit_field) => bit_field.deprecated_version,
            Record(ref rec) => rec.deprecated_version,
            Union(_) => None,
            Function(ref func) => func.deprecated_version,
            Interface(ref interface) => interface.deprecated_version,
            Array(_) => None,
            Class(ref class) => class.deprecated_version,
            Custom(_) => None,
            CArray(_) => None,
            FixedArray(..) => None,
            PtrArray(_) => None,
            HashTable(_, _) => None,
            List(_) => None,
            SList(_) => None,
        }
    }

    pub fn get_glib_name(&self) -> Option<&str> {
        use self::Type::*;
        match *self {
            Alias(ref alias) => Some(&alias.c_identifier),
            Enumeration(ref enum_) => Some(&enum_.c_type),
            Bitfield(ref bit_field) => Some(&bit_field.c_type),
            Record(ref rec) => Some(&rec.c_type),
            Union(ref union) => union.c_type.as_ref().map(|s| &s[..]),
            Function(ref func) => func.c_identifier.as_ref().map(|s| &s[..]),
            Interface(ref interface) => Some(&interface.c_type),
            Class(ref class) => Some(&class.c_type),
            _ => None,
        }
    }

    pub fn c_array(
        library: &mut Library,
        inner: TypeId,
        size: Option<u16>,
        c_type: Option<String>,
    ) -> TypeId {
        let name = Type::c_array_internal_name(inner, size, &c_type);
        if let Some(size) = size {
            library.add_type(
                INTERNAL_NAMESPACE,
                &name,
                Type::FixedArray(inner, size, c_type),
            )
        } else {
            library.add_type(INTERNAL_NAMESPACE, &name, Type::CArray(inner))
        }
    }

    pub fn find_c_array(library: &Library, inner: TypeId, size: Option<u16>) -> TypeId {
        let name = Type::c_array_internal_name(inner, size, &None);
        library
            .find_type(INTERNAL_NAMESPACE, &name)
            .unwrap_or_else(|| panic!("No type for '*.{}'", name))
    }

    fn c_array_internal_name(inner: TypeId, size: Option<u16>, c_type: &Option<String>) -> String {
        if let Some(size) = size {
            format!("[#{:?}; {};{:?}]", inner, size, c_type)
        } else {
            format!("[#{:?}]", inner)
        }
    }

    pub fn container(library: &mut Library, name: &str, mut inner: Vec<TypeId>) -> Option<TypeId> {
        match (name, inner.len()) {
            ("GLib.Array", 1) => {
                let tid = inner.remove(0);
                Some((format!("Array(#{:?})", tid), Type::Array(tid)))
            }
            ("GLib.PtrArray", 1) => {
                let tid = inner.remove(0);
                Some((format!("PtrArray(#{:?})", tid), Type::PtrArray(tid)))
            }
            ("GLib.HashTable", 2) => {
                let k_tid = inner.remove(0);
                let v_tid = inner.remove(0);
                Some((
                    format!("HashTable(#{:?}, #{:?})", k_tid, v_tid),
                    Type::HashTable(k_tid, v_tid),
                ))
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
        }
        .map(|(name, typ)| library.add_type(INTERNAL_NAMESPACE, &name, typ))
    }

    pub fn function(library: &mut Library, func: Function) -> TypeId {
        let mut param_tids: Vec<TypeId> = func.parameters.iter().map(|p| p.typ).collect();
        param_tids.push(func.ret.typ);
        let typ = Type::Function(func);
        library.add_type(INTERNAL_NAMESPACE, &format!("fn<#{:?}>", param_tids), typ)
    }

    pub fn union(library: &mut Library, u: Union, ns_id: u16) -> TypeId {
        let field_tids: Vec<TypeId> = u.fields.iter().map(|f| f.typ).collect();
        let typ = Type::Union(u);
        library.add_type(ns_id, &format!("#{:?}", field_tids), typ)
    }

    pub fn record(library: &mut Library, r: Record, ns_id: u16) -> TypeId {
        let field_tids: Vec<TypeId> = r.fields.iter().map(|f| f.typ).collect();
        let typ = Type::Record(r);
        library.add_type(ns_id, &format!("#{:?}", field_tids), typ)
    }

    pub fn functions(&self) -> &[Function] {
        match *self {
            Type::Enumeration(ref e) => &e.functions,
            Type::Bitfield(ref b) => &b.functions,
            Type::Record(ref r) => &r.functions,
            Type::Union(ref u) => &u.functions,
            Type::Interface(ref i) => &i.functions,
            Type::Class(ref c) => &c.functions,
            _ => &[],
        }
    }

    pub fn is_fundamental(&self) -> bool {
        match *self {
            Type::Fundamental(_) => true,
            _ => false,
        }
    }

    /// If the type is an Alias containing a fundamental, it'll return true (whereas
    /// `is_fundamental` won't).
    pub fn is_fundamental_type(&self, env: &Env) -> bool {
        match *self {
            Type::Alias(ref x) => env.library.type_(x.typ).is_fundamental_type(env),
            ref x => x.is_fundamental(),
        }
    }

    pub fn get_inner_type<'a>(&'a self, env: &'a Env) -> Option<(&'a Type, u16)> {
        match *self {
            Type::Array(t)
            | Type::CArray(t)
            | Type::FixedArray(t, ..)
            | Type::PtrArray(t)
            | Type::List(t)
            | Type::SList(t) => {
                let ty = env.type_(t);
                ty.get_inner_type(env).or_else(|| Some((ty, t.ns_id)))
            }
            _ => None,
        }
    }

    pub fn is_function(&self) -> bool {
        match *self {
            Type::Function(_) => true,
            _ => false,
        }
    }

    pub fn is_class(&self) -> bool {
        match *self {
            Type::Class(_) => true,
            _ => false,
        }
    }

    pub fn is_interface(&self) -> bool {
        match *self {
            Type::Interface(_) => true,
            _ => false,
        }
    }

    pub fn is_final_type(&self) -> bool {
        match *self {
            Type::Class(Class { final_type, .. }) => final_type,
            Type::Interface(..) => false,
            _ => true,
        }
    }
}

macro_rules! impl_maybe_ref {
    () => ();
    ($name:ident, $($more:ident,)*) => (
        impl_maybe_ref!($($more,)*);

        impl MaybeRef<$name> for Type {
            fn maybe_ref(&self) -> Option<&$name> {
                if let Type::$name(ref x) = *self { Some(x) } else { None }
            }

            fn to_ref(&self) -> &$name {
                self.maybe_ref().unwrap_or_else(|| {
                    panic!("{} is not a {}", self.get_name(), stringify!($name))
                })
            }
        }
    );
}

impl_maybe_ref!(
    Alias,
    Bitfield,
    Class,
    Enumeration,
    Function,
    Fundamental,
    Interface,
    Record,
    Union,
);

impl<U> MaybeRefAs for U {
    fn maybe_ref_as<T>(&self) -> Option<&T>
    where
        Self: MaybeRef<T>,
    {
        self.maybe_ref()
    }

    fn to_ref_as<T>(&self) -> &T
    where
        Self: MaybeRef<T>,
    {
        self.to_ref()
    }
}

#[derive(Debug, Default)]
pub struct Namespace {
    pub name: String,
    pub types: Vec<Option<Type>>,
    pub index: BTreeMap<String, u32>,
    pub glib_name_index: HashMap<String, u32>,
    pub constants: Vec<Constant>,
    pub functions: Vec<Function>,
    pub package_name: Option<String>,
    pub versions: BTreeSet<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
    pub shared_library: Vec<String>,
    pub identifier_prefixes: Vec<String>,
    pub symbol_prefixes: Vec<String>,
    /// C headers, relative to include directories provided by pkg-config --cflags.
    pub c_includes: Vec<String>,
}

impl Namespace {
    fn new(name: &str) -> Namespace {
        Namespace {
            name: name.into(),
            ..Namespace::default()
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

    fn type_mut(&mut self, id: u32) -> &mut Type {
        self.types[id as usize].as_mut().unwrap()
    }

    fn add_type(&mut self, name: &str, typ: Option<Type>) -> u32 {
        let glib_name = typ
            .as_ref()
            .and_then(Type::get_glib_name)
            .map(ToOwned::to_owned);
        let id = if let Some(id) = self.find_type(name) {
            self.types[id as usize] = typ;
            id
        } else {
            let id = self.types.len() as u32;
            self.types.push(typ);
            self.index.insert(name.into(), id);
            id
        };
        if let Some(s) = glib_name {
            self.glib_name_index.insert(s, id);
        }
        id
    }

    fn find_type(&self, name: &str) -> Option<u32> {
        self.index.get(name).cloned()
    }
}

pub const INTERNAL_NAMESPACE_NAME: &str = "*";
pub const INTERNAL_NAMESPACE: u16 = 0;
pub const MAIN_NAMESPACE: u16 = 1;

#[derive(Debug)]
pub struct Library {
    pub namespaces: Vec<Namespace>,
    pub index: HashMap<String, u16>,
}

impl Library {
    pub fn new(main_namespace_name: &str) -> Library {
        let mut library = Library {
            namespaces: Vec::new(),
            index: HashMap::new(),
        };
        assert_eq!(
            INTERNAL_NAMESPACE,
            library.add_namespace(INTERNAL_NAMESPACE_NAME)
        );
        for &(name, t) in FUNDAMENTAL {
            library.add_type(INTERNAL_NAMESPACE, name, Type::Fundamental(t));
        }
        assert_eq!(MAIN_NAMESPACE, library.add_namespace(main_namespace_name));

        //For string_type override
        Type::c_array(&mut library, TypeId::tid_utf8(), None, None);
        Type::c_array(&mut library, TypeId::tid_filename(), None, None);
        Type::c_array(&mut library, TypeId::tid_os_string(), None, None);

        library
    }

    pub fn show_non_bound_types(&self, env: &Env) {
        let not_allowed_ending = [
            "Class",
            "Private",
            "Func",
            "Callback",
            "Accessible",
            "Iface",
            "Type",
        ];
        let namespace_name = self.namespaces[MAIN_NAMESPACE as usize].name.clone();
        let mut parents = HashSet::new();

        for x in &self.namespace(MAIN_NAMESPACE).types {
            if let Some(ref x) = *x {
                let name = x.get_name();
                let full_name = format!("{}.{}", namespace_name, name);
                let mut check_methods = true;

                if !not_allowed_ending.iter().any(|s| name.ends_with(s)) {
                    let version = x.get_deprecated_version();
                    let depr_version = version.unwrap_or(env.config.min_cfg_version);
                    if !env.analysis.objects.contains_key(&full_name)
                        && !env.analysis.records.contains_key(&full_name)
                        && !env.config.objects.iter().any(|o| o.1.name == full_name)
                        && depr_version >= env.config.min_cfg_version
                    {
                        check_methods = false;
                        if let Some(version) = version {
                            println!("[NOT GENERATED] {} (deprecated in {})", full_name, version);
                        } else {
                            println!("[NOT GENERATED] {}", full_name);
                        }
                    } else if let Type::Class(Class { properties, .. }) = x {
                        let builder_name = format!("{}Builder", full_name);
                        if !env.config.objects.contains_key(&builder_name)
                            && properties
                                .iter()
                                .any(|prop| prop.construct_only || prop.construct || prop.writable)
                        {
                            println!("[NOT GENERATED BUILDER] {}", builder_name);
                        }
                    }
                }
                if let Some(tid) = env.library.find_type(0, &full_name) {
                    let gobject_id = env.library.find_type(0, "GObject.Object").unwrap();

                    for &super_tid in env.class_hierarchy.supertypes(tid) {
                        let ty = env.library.type_(super_tid);
                        let ns_id = super_tid.ns_id as usize;
                        let full_parent_name =
                            format!("{}.{}", self.namespaces[ns_id].name, ty.get_name());
                        if super_tid != gobject_id
                            && env
                                .type_status(&super_tid.full_name(&env.library))
                                .ignored()
                            && parents.insert(full_parent_name.clone())
                        {
                            if let Some(version) = ty.get_deprecated_version() {
                                println!(
                                    "[NOT GENERATED PARENT] {} (deprecated in {})",
                                    full_parent_name, version
                                );
                            } else {
                                println!("[NOT GENERATED PARENT] {}", full_parent_name);
                            }
                        }
                    }
                    if check_methods {
                        self.not_bound_functions(
                            env,
                            &format!("{}::", full_name),
                            x.functions(),
                            "METHOD",
                        );
                    }
                }
            }
        }
        self.not_bound_functions(
            env,
            &format!("{}.", namespace_name),
            &self.namespace(MAIN_NAMESPACE).functions,
            "FUNCTION",
        );
    }

    fn not_bound_functions(&self, env: &Env, prefix: &str, functions: &[Function], kind: &str) {
        for func in functions {
            let version = func.deprecated_version;
            let depr_version = version.unwrap_or(env.config.min_cfg_version);

            if depr_version < env.config.min_cfg_version {
                continue;
            }

            let mut errors = func
                .parameters
                .iter()
                .filter_map(|p| {
                    let mut ty = env.library.type_(p.typ);
                    let mut ns_id = p.typ.ns_id as usize;
                    if let Some((t, n)) = ty.get_inner_type(env) {
                        ty = t;
                        ns_id = n as usize;
                    }
                    if ty.is_fundamental() {
                        return None;
                    }
                    let full_name = format!("{}.{}", self.namespaces[ns_id].name, ty.get_name());
                    if env.type_status(&p.typ.full_name(&env.library)).ignored()
                        && !env.analysis.objects.contains_key(&full_name)
                        && !env.analysis.records.contains_key(&full_name)
                        && !env.config.objects.iter().any(|o| o.1.name == full_name)
                    {
                        Some(full_name)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            {
                let mut ty = env.library.type_(func.ret.typ);
                let mut ns_id = func.ret.typ.ns_id as usize;
                if let Some((t, n)) = ty.get_inner_type(env) {
                    ty = t;
                    ns_id = n as usize;
                }
                if !ty.is_fundamental() {
                    let full_name = format!("{}.{}", self.namespaces[ns_id].name, ty.get_name());
                    if env
                        .type_status(&func.ret.typ.full_name(&env.library))
                        .ignored()
                        && !env.analysis.objects.contains_key(&full_name)
                        && !env.analysis.records.contains_key(&full_name)
                        && !env.config.objects.iter().any(|o| o.1.name == full_name)
                    {
                        errors.push(full_name);
                    }
                }
            }
            if !errors.is_empty() {
                let full_name = format!("{}{}", prefix, func.name);
                let deprecated_version = match version {
                    Some(dv) => format!(" (deprecated in {})", dv),
                    None => String::new(),
                };
                if errors.len() > 1 {
                    let end = errors.pop().unwrap();
                    let begin = errors.join(", ");
                    println!(
                        "[NOT GENERATED {}] {}{} because of {} and {}",
                        kind, full_name, deprecated_version, begin, end
                    );
                } else {
                    println!(
                        "[NOT GENERATED {}] {}{} because of {}",
                        kind, full_name, deprecated_version, errors[0]
                    );
                }
            }
        }
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
        } else {
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
        TypeId {
            ns_id,
            id: self.namespace_mut(ns_id).add_type(name, Some(typ)),
        }
    }

    pub fn find_type(&self, current_ns_id: u16, name: &str) -> Option<TypeId> {
        let (mut ns, name) = split_namespace_name(name);
        if name == "GType" {
            ns = None;
        }

        if let Some(ns) = ns {
            self.find_namespace(ns).and_then(|ns_id| {
                self.namespace(ns_id)
                    .find_type(name)
                    .map(|id| TypeId { ns_id, id })
            })
        } else if let Some(id) = self.namespace(current_ns_id).find_type(name) {
            Some(TypeId {
                ns_id: current_ns_id,
                id,
            })
        } else if let Some(id) = self.namespace(INTERNAL_NAMESPACE).find_type(name) {
            Some(TypeId {
                ns_id: INTERNAL_NAMESPACE,
                id,
            })
        } else {
            None
        }
    }

    pub fn find_or_stub_type(&mut self, current_ns_id: u16, name: &str) -> TypeId {
        if let Some(tid) = self.find_type(current_ns_id, name) {
            return tid;
        }

        let (ns, name) = split_namespace_name(name);

        if let Some(ns) = ns {
            let ns_id = self
                .find_namespace(ns)
                .unwrap_or_else(|| self.add_namespace(ns));
            let ns = self.namespace_mut(ns_id);
            let id = ns
                .find_type(name)
                .unwrap_or_else(|| ns.add_type(name, None));
            return TypeId { ns_id, id };
        }

        let id = self.namespace_mut(current_ns_id).add_type(name, None);
        TypeId {
            ns_id: current_ns_id,
            id,
        }
    }

    pub fn type_(&self, tid: TypeId) -> &Type {
        self.namespace(tid.ns_id).type_(tid.id)
    }

    pub fn type_mut(&mut self, tid: TypeId) -> &mut Type {
        self.namespace_mut(tid.ns_id).type_mut(tid.id)
    }

    pub fn register_version(&mut self, ns_id: u16, version: Version) {
        self.namespace_mut(ns_id).versions.insert(version);
    }

    pub fn types<'a>(&'a self) -> Box<dyn Iterator<Item = (TypeId, &Type)> + 'a> {
        Box::new(self.namespaces.iter().enumerate().flat_map(|(ns_id, ns)| {
            ns.types.iter().enumerate().filter_map(move |(id, type_)| {
                let tid = TypeId {
                    ns_id: ns_id as u16,
                    id: id as u32,
                };
                type_.as_ref().map(|t| (tid, t))
            })
        }))
    }

    /// Types from a single namespace in alphabetical order.
    pub fn namespace_types<'a>(
        &'a self,
        ns_id: u16,
    ) -> Box<dyn Iterator<Item = (TypeId, &Type)> + 'a> {
        let ns = self.namespace(ns_id);
        Box::new(ns.index.values().map(move |&id| {
            (
                TypeId { ns_id, id },
                ns.types[id as usize].as_ref().unwrap(),
            )
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fundamental_tids() {
        let lib = Library::new("Gtk");

        assert_eq!(TypeId::tid_none().full_name(&lib), "*.None");
        assert_eq!(TypeId::tid_bool().full_name(&lib), "*.Boolean");
        assert_eq!(TypeId::tid_utf8().full_name(&lib), "*.Utf8");
        assert_eq!(TypeId::tid_filename().full_name(&lib), "*.Filename");
        assert_eq!(TypeId::tid_os_string().full_name(&lib), "*.OsString");
    }

}
