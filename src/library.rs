use std::{
    cmp::{Ord, Ordering, PartialOrd},
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fmt,
    iter::Iterator,
    ops::{Deref, DerefMut},
    str::FromStr,
};

use crate::{
    analysis::conversion_type::ConversionType, config::gobjects::GStatus, env::Env,
    nameutil::split_namespace_name, traits::*, version::Version,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transfer {
    None,
    Container,
    Full,
}

impl FromStr for Transfer {
    type Err = String;
    fn from_str(name: &str) -> Result<Self, String> {
        match name {
            "none" => Ok(Self::None),
            "container" => Ok(Self::Container),
            "full" => Ok(Self::Full),
            _ => Err(format!("Unknown ownership transfer mode '{name}'")),
        }
    }
}

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterDirection {
    None,
    #[default]
    In,
    Out,
    InOut,
    Return,
}

impl ParameterDirection {
    pub fn is_in(self) -> bool {
        matches!(self, Self::In | Self::InOut)
    }

    pub fn is_out(self) -> bool {
        matches!(self, Self::Out | Self::InOut)
    }
}

impl FromStr for ParameterDirection {
    type Err = String;
    fn from_str(name: &str) -> Result<Self, String> {
        match name {
            "in" => Ok(Self::In),
            "out" => Ok(Self::Out),
            "inout" => Ok(Self::InOut),
            _ => Err(format!("Unknown parameter direction '{name}'")),
        }
    }
}

/// Annotation describing lifetime requirements / guarantees of callback
/// parameters, that is callback itself and associated user data.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterScope {
    /// Parameter is not of callback type.
    #[default]
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
        matches!(self, Self::Call)
    }

    pub fn is_async(self) -> bool {
        matches!(self, Self::Async)
    }

    pub fn is_none(self) -> bool {
        matches!(self, Self::None)
    }
}

impl FromStr for ParameterScope {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, String> {
        match name {
            "call" => Ok(Self::Call),
            "async" => Ok(Self::Async),
            "notified" => Ok(Self::Notified),
            _ => Err(format!("Unknown parameter scope type: {name}")),
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
pub struct Mandatory(pub bool);

impl Deref for Mandatory {
    type Target = bool;
    fn deref(&self) -> &bool {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Infallible(pub bool);

impl Deref for Infallible {
    type Target = bool;
    fn deref(&self) -> &bool {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionKind {
    Constructor,
    Function,
    Method,
    Global,
    ClassMethod,
    VirtualMethod,
}

impl FromStr for FunctionKind {
    type Err = String;
    fn from_str(name: &str) -> Result<Self, String> {
        match name {
            "constructor" => Ok(Self::Constructor),
            "function" => Ok(Self::Function),
            "method" => Ok(Self::Method),
            "callback" => Ok(Self::Function),
            "global" => Ok(Self::Global),
            _ => Err(format!("Unknown function kind '{name}'")),
        }
    }
}

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Concurrency {
    #[default]
    None,
    Send,
    SendSync,
}

impl FromStr for Concurrency {
    type Err = String;
    fn from_str(name: &str) -> Result<Self, String> {
        match name {
            "none" => Ok(Self::None),
            "send" => Ok(Self::Send),
            "send+sync" => Ok(Self::SendSync),
            _ => Err(format!("Unknown concurrency kind '{name}'")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basic {
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
    // Same encoding as Filename but can contains any string
    // Not defined in GLib directly
    OsString,
    Bool,
    Unsupported,
}

impl Basic {
    pub fn requires_conversion(&self) -> bool {
        !matches!(
            self,
            Self::Int8
                | Self::UInt8
                | Self::Int16
                | Self::UInt16
                | Self::Int32
                | Self::UInt32
                | Self::Int64
                | Self::UInt64
                | Self::Char
                | Self::UChar
                | Self::Short
                | Self::UShort
                | Self::Int
                | Self::UInt
                | Self::Long
                | Self::ULong
                | Self::Size
                | Self::SSize
                | Self::Float
                | Self::Double
                | Self::Bool
        )
    }
}

const BASIC: &[(&str, Basic)] = &[
    ("none", Basic::None),
    ("gboolean", Basic::Boolean),
    ("gint8", Basic::Int8),
    ("guint8", Basic::UInt8),
    ("gint16", Basic::Int16),
    ("guint16", Basic::UInt16),
    ("gint32", Basic::Int32),
    ("guint32", Basic::UInt32),
    ("gint64", Basic::Int64),
    ("guint64", Basic::UInt64),
    ("gchar", Basic::Char),
    ("guchar", Basic::UChar),
    ("gshort", Basic::Short),
    ("gushort", Basic::UShort),
    ("gint", Basic::Int),
    ("guint", Basic::UInt),
    ("glong", Basic::Long),
    ("gulong", Basic::ULong),
    ("gsize", Basic::Size),
    ("gssize", Basic::SSize),
    ("gfloat", Basic::Float),
    ("gdouble", Basic::Double),
    ("long double", Basic::Unsupported),
    ("gunichar", Basic::UniChar),
    ("gconstpointer", Basic::Pointer),
    ("gpointer", Basic::Pointer),
    ("va_list", Basic::Unsupported),
    ("varargs", Basic::VarArgs),
    ("utf8", Basic::Utf8),
    ("filename", Basic::Filename),
    ("GType", Basic::Type),
    ("gintptr", Basic::IntPtr),
    ("guintptr", Basic::UIntPtr),
    // TODO: this is temporary name, change it when type added to GLib
    ("os_string", Basic::OsString),
    ("bool", Basic::Bool),
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

    pub fn tid_uint32() -> TypeId {
        TypeId { ns_id: 0, id: 7 }
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

    pub fn tid_c_bool() -> TypeId {
        TypeId { ns_id: 0, id: 34 }
    }

    pub fn is_basic_type(self, env: &Env) -> bool {
        env.library.type_(self).is_basic_type(env)
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
    pub doc_deprecated: Option<String>,
    pub status: GStatus,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
}

#[derive(Debug)]
pub enum ErrorDomain {
    Quark(String),
    Function(String),
}

#[derive(Debug)]
pub struct Enumeration {
    pub name: String,
    pub c_type: String,
    pub symbol_prefix: Option<String>,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
    pub error_domain: Option<ErrorDomain>,
    pub glib_get_type: Option<String>,
}

#[derive(Debug)]
pub struct Bitfield {
    pub name: String,
    pub c_type: String,
    pub symbol_prefix: Option<String>,
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
    pub symbol_prefix: Option<String>,
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

impl Record {
    pub fn has_free(&self) -> bool {
        self.functions.iter().any(|f| f.name == "free") || (self.has_copy() && self.has_destroy())
    }

    pub fn has_copy(&self) -> bool {
        self.functions
            .iter()
            .any(|f| f.name == "copy" || f.name == "copy_into")
    }

    pub fn has_destroy(&self) -> bool {
        self.functions.iter().any(|f| f.name == "destroy")
    }

    pub fn has_unref(&self) -> bool {
        self.functions.iter().any(|f| f.name == "unref")
    }

    pub fn has_ref(&self) -> bool {
        self.functions.iter().any(|f| f.name == "ref")
    }
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
    pub symbol_prefix: Option<String>,
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
    pub array_length: Option<u32>,
    pub is_error: bool,
    pub doc: Option<String>,
    pub scope: ParameterScope,
    /// Index of the user data parameter associated with the callback.
    pub closure: Option<usize>,
    /// Index of the destroy notification parameter associated with the
    /// callback.
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
    pub is_detailed: bool,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
}

#[derive(Default, Debug)]
pub struct Interface {
    pub name: String,
    pub c_type: String,
    pub symbol_prefix: String,
    pub type_struct: Option<String>,
    pub c_class_type: Option<String>,
    pub glib_get_type: String,
    pub functions: Vec<Function>,
    pub virtual_methods: Vec<Function>,
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
    pub symbol_prefix: String,
    pub type_struct: Option<String>,
    pub c_class_type: Option<String>,
    pub glib_get_type: String,
    pub fields: Vec<Field>,
    pub functions: Vec<Function>,
    pub virtual_methods: Vec<Function>,
    pub signals: Vec<Signal>,
    pub properties: Vec<Property>,
    pub parent: Option<TypeId>,
    pub implements: Vec<TypeId>,
    pub final_type: bool,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
    pub is_abstract: bool,
    pub is_fundamental: bool,
    /// Specific to fundamental types
    pub ref_fn: Option<String>,
    pub unref_fn: Option<String>,
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

#[derive(Debug, Eq, PartialEq)]
pub enum Type {
    Basic(Basic),
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
        f.write_str(match self {
            Self::Basic(_) => "Basic",
            Self::Alias(_) => "Alias",
            Self::Enumeration(_) => "Enumeration",
            Self::Bitfield(_) => "Bitfield",
            Self::Record(_) => "Record",
            Self::Union(_) => "Union",
            Self::Function(_) => "Function",
            Self::Interface(_) => "Interface",
            Self::Class(_) => "Class",
            Self::Custom(_) => "Custom",
            Self::Array(_) => "Array",
            Self::CArray(_) => "CArray",
            Self::FixedArray(_, _, _) => "FixedArray",
            Self::PtrArray(_) => "PtrArray",
            Self::HashTable(_, _) => "HashTable",
            Self::List(_) => "List",
            Self::SList(_) => "SList",
        })
    }
}

impl Type {
    pub fn get_name(&self) -> String {
        match self {
            Self::Basic(basic) => format!("{basic:?}"),
            Self::Alias(alias) => alias.name.clone(),
            Self::Enumeration(enum_) => enum_.name.clone(),
            Self::Bitfield(bit_field) => bit_field.name.clone(),
            Self::Record(rec) => rec.name.clone(),
            Self::Union(u) => u.name.clone(),
            Self::Function(func) => func.name.clone(),
            Self::Interface(interface) => interface.name.clone(),
            Self::Array(type_id) => format!("Array {type_id:?}"),
            Self::Class(class) => class.name.clone(),
            Self::Custom(custom) => custom.name.clone(),
            Self::CArray(type_id) => format!("CArray {type_id:?}"),
            Self::FixedArray(type_id, size, _) => format!("FixedArray {type_id:?}; {size}"),
            Self::PtrArray(type_id) => format!("PtrArray {type_id:?}"),
            Self::HashTable(key_type_id, value_type_id) => {
                format!("HashTable {key_type_id:?}/{value_type_id:?}")
            }
            Self::List(type_id) => format!("List {type_id:?}"),
            Self::SList(type_id) => format!("SList {type_id:?}"),
        }
    }

    pub fn get_deprecated_version(&self) -> Option<Version> {
        match self {
            Self::Basic(_) => None,
            Self::Alias(_) => None,
            Self::Enumeration(enum_) => enum_.deprecated_version,
            Self::Bitfield(bit_field) => bit_field.deprecated_version,
            Self::Record(rec) => rec.deprecated_version,
            Self::Union(_) => None,
            Self::Function(func) => func.deprecated_version,
            Self::Interface(interface) => interface.deprecated_version,
            Self::Array(_) => None,
            Self::Class(class) => class.deprecated_version,
            Self::Custom(_) => None,
            Self::CArray(_) => None,
            Self::FixedArray(..) => None,
            Self::PtrArray(_) => None,
            Self::HashTable(_, _) => None,
            Self::List(_) => None,
            Self::SList(_) => None,
        }
    }

    pub fn get_glib_name(&self) -> Option<&str> {
        match self {
            Self::Alias(alias) => Some(&alias.c_identifier),
            Self::Enumeration(enum_) => Some(&enum_.c_type),
            Self::Bitfield(bit_field) => Some(&bit_field.c_type),
            Self::Record(rec) => Some(&rec.c_type),
            Self::Union(union) => union.c_type.as_deref(),
            Self::Function(func) => func.c_identifier.as_deref(),
            Self::Interface(interface) => Some(&interface.c_type),
            Self::Class(class) => Some(&class.c_type),
            _ => None,
        }
    }

    pub fn c_array(
        library: &mut Library,
        inner: TypeId,
        size: Option<u16>,
        c_type: Option<String>,
    ) -> TypeId {
        let name = Self::c_array_internal_name(inner, size, &c_type);
        if let Some(size) = size {
            library.add_type(
                INTERNAL_NAMESPACE,
                &name,
                Self::FixedArray(inner, size, c_type),
            )
        } else {
            library.add_type(INTERNAL_NAMESPACE, &name, Self::CArray(inner))
        }
    }

    pub fn find_c_array(library: &Library, inner: TypeId, size: Option<u16>) -> TypeId {
        let name = Self::c_array_internal_name(inner, size, &None);
        library
            .find_type(INTERNAL_NAMESPACE, &name)
            .unwrap_or_else(|| panic!("No type for '*.{name}'"))
    }

    fn c_array_internal_name(inner: TypeId, size: Option<u16>, c_type: &Option<String>) -> String {
        if let Some(size) = size {
            format!("[#{inner:?}; {size};{c_type:?}]")
        } else {
            format!("[#{inner:?}]")
        }
    }

    pub fn container(library: &mut Library, name: &str, mut inner: Vec<TypeId>) -> Option<TypeId> {
        match (name, inner.len()) {
            ("GLib.Array", 1) => {
                let tid = inner.remove(0);
                Some((format!("Array(#{tid:?})"), Self::Array(tid)))
            }
            ("GLib.PtrArray", 1) => {
                let tid = inner.remove(0);
                Some((format!("PtrArray(#{tid:?})"), Self::PtrArray(tid)))
            }
            ("GLib.HashTable", 2) => {
                let k_tid = inner.remove(0);
                let v_tid = inner.remove(0);
                Some((
                    format!("HashTable(#{k_tid:?}, #{v_tid:?})"),
                    Self::HashTable(k_tid, v_tid),
                ))
            }
            ("GLib.List", 1) => {
                let tid = inner.remove(0);
                Some((format!("List(#{tid:?})"), Self::List(tid)))
            }
            ("GLib.SList", 1) => {
                let tid = inner.remove(0);
                Some((format!("SList(#{tid:?})"), Self::SList(tid)))
            }
            _ => None,
        }
        .map(|(name, typ)| library.add_type(INTERNAL_NAMESPACE, &name, typ))
    }

    pub fn function(library: &mut Library, func: Function) -> TypeId {
        let mut param_tids: Vec<TypeId> = func.parameters.iter().map(|p| p.typ).collect();
        param_tids.push(func.ret.typ);
        let typ = Self::Function(func);
        library.add_type(INTERNAL_NAMESPACE, &format!("fn<#{param_tids:?}>"), typ)
    }

    pub fn union(library: &mut Library, u: Union, ns_id: u16) -> TypeId {
        let field_tids: Vec<TypeId> = u.fields.iter().map(|f| f.typ).collect();
        let typ = Self::Union(u);
        library.add_type(ns_id, &format!("#{field_tids:?}"), typ)
    }

    pub fn record(library: &mut Library, r: Record, ns_id: u16) -> TypeId {
        let field_tids: Vec<TypeId> = r.fields.iter().map(|f| f.typ).collect();
        let typ = Self::Record(r);
        library.add_type(ns_id, &format!("#{field_tids:?}"), typ)
    }

    pub fn functions(&self) -> &[Function] {
        match self {
            Self::Enumeration(e) => &e.functions,
            Self::Bitfield(b) => &b.functions,
            Self::Record(r) => &r.functions,
            Self::Union(u) => &u.functions,
            Self::Interface(i) => &i.functions,
            Self::Class(c) => &c.functions,
            _ => &[],
        }
    }

    pub fn is_basic(&self) -> bool {
        matches!(*self, Self::Basic(_))
    }

    /// If the type is an Alias containing a basic, it'll return true (whereas
    /// `is_basic` won't).
    pub fn is_basic_type(&self, env: &Env) -> bool {
        match self {
            Self::Alias(x) => env.library.type_(x.typ).is_basic_type(env),
            x => x.is_basic(),
        }
    }

    pub fn get_inner_type<'a>(&'a self, env: &'a Env) -> Option<(&'a Type, u16)> {
        match *self {
            Self::Array(t)
            | Self::CArray(t)
            | Self::FixedArray(t, ..)
            | Self::PtrArray(t)
            | Self::List(t)
            | Self::SList(t) => {
                let ty = env.type_(t);
                ty.get_inner_type(env).or(Some((ty, t.ns_id)))
            }
            _ => None,
        }
    }

    pub fn is_function(&self) -> bool {
        matches!(*self, Self::Function(_))
    }

    pub fn is_class(&self) -> bool {
        matches!(*self, Self::Class(_))
    }

    pub fn is_interface(&self) -> bool {
        matches!(*self, Self::Interface(_))
    }

    pub fn is_final_type(&self) -> bool {
        match *self {
            Self::Class(Class { final_type, .. }) => final_type,
            Self::Interface(..) => false,
            _ => true,
        }
    }

    pub fn is_fundamental(&self) -> bool {
        match *self {
            Self::Class(Class { is_fundamental, .. }) => is_fundamental,
            _ => false,
        }
    }

    pub fn is_abstract(&self) -> bool {
        match *self {
            Self::Class(Class { is_abstract, .. }) => is_abstract,
            _ => false,
        }
    }

    pub fn is_enumeration(&self) -> bool {
        matches!(*self, Self::Enumeration(_))
    }

    pub fn is_bitfield(&self) -> bool {
        matches!(*self, Self::Bitfield(_))
    }
}

macro_rules! impl_maybe_ref {
    () => ();
    ($name:ident, $($more:ident,)*) => (
        impl_maybe_ref!($($more,)*);

        impl MaybeRef<$name> for Type {
            fn maybe_ref(&self) -> Option<&$name> {
                if let Self::$name(x) = self { Some(x) } else { None }
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
    Basic,
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
    pub package_names: Vec<String>,
    pub versions: BTreeSet<Version>,
    pub doc: Option<String>,
    pub doc_deprecated: Option<String>,
    pub shared_library: Vec<String>,
    pub identifier_prefixes: Vec<String>,
    pub symbol_prefixes: Vec<String>,
    /// C headers, relative to include directories provided by pkg-config
    /// --cflags.
    pub c_includes: Vec<String>,
}

impl Namespace {
    fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
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
        self.index.get(name).copied()
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
    pub fn new(main_namespace_name: &str) -> Self {
        let mut library = Self {
            namespaces: Vec::new(),
            index: HashMap::new(),
        };
        assert_eq!(
            INTERNAL_NAMESPACE,
            library.add_namespace(INTERNAL_NAMESPACE_NAME)
        );
        for &(name, t) in BASIC {
            library.add_type(INTERNAL_NAMESPACE, name, Type::Basic(t));
        }
        assert_eq!(MAIN_NAMESPACE, library.add_namespace(main_namespace_name));

        // For string_type override
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
            "Interface",
        ];
        let namespace_name = self.namespaces[MAIN_NAMESPACE as usize].name.clone();
        let mut parents = HashSet::new();

        for x in self.namespace(MAIN_NAMESPACE).types.iter().flatten() {
            let name = x.get_name();
            let full_name = format!("{namespace_name}.{name}");
            let mut check_methods = true;

            if !not_allowed_ending.iter().any(|s| name.ends_with(s))
                || x.is_enumeration()
                || x.is_bitfield()
            {
                let version = x.get_deprecated_version();
                let depr_version = version.unwrap_or(env.config.min_cfg_version);
                if !env.analysis.objects.contains_key(&full_name)
                    && !env.analysis.records.contains_key(&full_name)
                    && !env.config.objects.iter().any(|o| o.1.name == full_name)
                    && depr_version >= env.config.min_cfg_version
                {
                    check_methods = false;
                    if let Some(version) = version {
                        println!("[NOT GENERATED] {full_name} (deprecated in {version})");
                    } else {
                        println!("[NOT GENERATED] {full_name}");
                    }
                } else if let Type::Class(Class { properties, .. }) = x {
                    if !env
                        .config
                        .objects
                        .get(&full_name)
                        .map_or(false, |obj| obj.generate_builder)
                        && properties
                            .iter()
                            .any(|prop| prop.construct_only || prop.construct || prop.writable)
                    {
                        println!("[NOT GENERATED BUILDER] {full_name}Builder");
                    }
                }
            }
            if let (Some(tid), Some(gobject_id)) = (
                env.library.find_type(0, &full_name),
                env.library.find_type(0, "GObject.Object"),
            ) {
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
                                "[NOT GENERATED PARENT] {full_parent_name} (deprecated in {version})"
                            );
                        } else {
                            println!("[NOT GENERATED PARENT] {full_parent_name}");
                        }
                    }
                }
                if check_methods {
                    self.not_bound_functions(
                        env,
                        &format!("{full_name}::"),
                        x.functions(),
                        "METHOD",
                    );
                }
            }
        }
        self.not_bound_functions(
            env,
            &format!("{namespace_name}."),
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
                    if ty.is_basic() {
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
                if !ty.is_basic() {
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
                    Some(dv) => format!(" (deprecated in {dv})"),
                    None => String::new(),
                };
                if errors.len() > 1 {
                    let end = errors.pop().unwrap();
                    let begin = errors.join(", ");
                    println!(
                        "[NOT GENERATED {kind}] {full_name}{deprecated_version} because of {begin} and {end}"
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
        self.index.get(name).copied()
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

    #[allow(clippy::manual_map)]
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

    pub fn is_crate(&self, crate_name: &str) -> bool {
        self.namespace(MAIN_NAMESPACE).name == crate_name
    }

    pub fn is_glib_crate(&self) -> bool {
        self.is_crate("GObject") || self.is_crate("GLib")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_tids() {
        let lib = Library::new("Gtk");

        assert_eq!(TypeId::tid_none().full_name(&lib), "*.None");
        assert_eq!(TypeId::tid_bool().full_name(&lib), "*.Boolean");
        assert_eq!(TypeId::tid_uint32().full_name(&lib), "*.UInt32");
        assert_eq!(TypeId::tid_c_bool().full_name(&lib), "*.Bool");
        assert_eq!(TypeId::tid_utf8().full_name(&lib), "*.Utf8");
        assert_eq!(TypeId::tid_filename().full_name(&lib), "*.Filename");
        assert_eq!(TypeId::tid_os_string().full_name(&lib), "*.OsString");
    }
}
