use std::cmp::{Ord, Ordering, PartialOrd};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use nameutil::split_namespace_name;
use traits::*;

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
    pub fn is_out(&self) -> bool {
        self == &ParameterDirection::Out || self == &ParameterDirection::InOut
    }
    pub fn can_as_return(&self) -> bool {
        self == &ParameterDirection::Out
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
    Unsupported,
}

pub const FUNDAMENTAL: [(&'static str, Fundamental); 31] = [
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
];

//default = "*.None"
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub struct TypeId {
    pub ns_id: u16,
    pub id: u32,
}

impl TypeId {
    pub fn full_name(&self, library: &Library) -> String{
        let ns_name = &library.namespace(self.ns_id).name;
        let type_ = &library.type_(*self);
        format!("{}.{}", ns_name, &type_.get_name()).into()
    }
}

impl PartialOrd for TypeId {
    fn partial_cmp(&self, other: &TypeId) -> Option<Ordering> {
        match self.ns_id.partial_cmp(&other.ns_id) {
            Some(Ordering::Equal) => self.id.partial_cmp(&other.id),
            x => x,
        }
    }
}

impl Ord for TypeId {
    fn cmp(&self, other: &TypeId) -> Ordering {
        match self.ns_id.cmp(&other.ns_id) {
            Ordering::Equal => self.id.cmp(&other.id),
            x => x,
        }
    }
}

pub struct Alias {
    pub name: String,
    pub c_identifier: String,
    pub typ: TypeId,
    pub target_c_type: String,
}

pub struct Constant {
    pub name: String,
    pub c_identifier: String,
    pub typ: TypeId,
    pub c_type: String,
    pub value: String,
}

pub struct Member {
    pub name: String,
    pub c_identifier: String,
    pub value: String,
}

pub struct Enumeration {
    pub name: String,
    pub c_type: String,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
}

pub struct Bitfield {
    pub name: String,
    pub c_type: String,
    pub members: Vec<Member>,
    pub functions: Vec<Function>,
}

pub struct Record {
    pub name: String,
    pub c_type: String,
    pub fields: Vec<Field>,
    pub functions: Vec<Function>,
}

#[derive(Default)]
pub struct Field {
    pub name: String,
    pub typ: TypeId,
    pub c_type: Option<String>,
    pub private: bool,
    pub bits: Option<u8>,
}

#[derive(Default)]
pub struct Union {
    pub name: String,
    pub c_type: Option<String>,
    pub fields: Vec<Field>,
    pub functions: Vec<Function>,
}

#[derive(Clone)]
pub struct Parameter {
    pub name: String,
    pub typ: TypeId,
    pub c_type: String,
    pub instance_parameter: bool,
    pub direction: ParameterDirection,
    pub transfer: Transfer,
    pub nullable: bool,
    pub allow_none: bool,
}

pub struct Function {
    pub name: String,
    pub c_identifier: Option<String>,
    pub kind: FunctionKind,
    pub parameters: Vec<Parameter>,
    pub ret: Parameter,
    pub throws: bool,
}

#[derive(Default)]
pub struct Interface {
    pub name: String,
    pub c_type: String,
    pub glib_get_type: String,
    pub functions: Vec<Function>,
    pub prerequisites: Vec<TypeId>,
    pub prereq_parents: Vec<TypeId>,
}

#[derive(Default)]
pub struct Class {
    pub name: String,
    pub c_type: String,
    pub glib_get_type: String,
    pub functions: Vec<Function>,
    pub parent: Option<TypeId>,
    pub parents: Vec<TypeId>,
    pub children: HashSet<TypeId>,
    pub implements: Vec<TypeId>,
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
);

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
    Array(TypeId),
    CArray(TypeId),
    FixedArray(TypeId, u16),
    PtrArray(TypeId),
    HashTable(TypeId, TypeId),
    List(TypeId),
    SList(TypeId),
}

impl Type {
    //others that Library and Parser must use analysis::rust_type::ToRustType
    pub fn get_name(&self) -> String {
        use self::Type::*;
        match self {
            &Fundamental(fund) => format!("{:?}", fund).into(),
            &Alias(ref alias) => alias.name.clone(),
            &Enumeration(ref enum_) => enum_.name.clone(),
            &Bitfield(ref bit_field) => bit_field.name.clone(),
            &Record(ref rec) => rec.name.clone(),
            &Union(ref union) => union.name.clone(),
            &Function(ref func) => func.name.clone(),
            &Interface(ref interface) => interface.name.clone(),
            &Array(type_id) => format!("Array {:?}", type_id),
            &Class(ref class) => class.name.clone(),
            &CArray(type_id) => format!("CArray {:?}", type_id),
            &FixedArray(type_id, size) => format!("FixedArray {:?}; {}", type_id, size),
            &PtrArray(type_id) => format!("PtrArray {:?}", type_id),
            &HashTable(key_type_id, value_type_id) => format!("HashTable {:?}/{:?}", key_type_id, value_type_id),
            &List(type_id) => format!("List {:?}", type_id),
            &SList(type_id) => format!("SList {:?}", type_id),
        }
    }

    pub fn get_glib_name(&self) -> Option<&str> {
        use self::Type::*;
        match self {
            &Alias(ref alias) => Some(&alias.c_identifier),
            &Enumeration(ref enum_) => Some(&enum_.c_type),
            &Bitfield(ref bit_field) => Some(&bit_field.c_type),
            &Record(ref rec) => Some(&rec.c_type),
            &Union(ref union) => union.c_type.as_ref().map(|s| &s[..]),
            &Function(ref func) => func.c_identifier.as_ref().map(|s| &s[..]),
            &Interface(ref interface) => Some(&interface.c_type),
            &Class(ref class) => Some(&class.c_type),
            _ => None,
        }
    }

    pub fn c_array(library: &mut Library, inner: TypeId, size: Option<u16>) -> TypeId {
        if let Some(size) = size {
            library.add_type(INTERNAL_NAMESPACE, &format!("[#{:?}; {}]", inner, size),
                             Type::FixedArray(inner, size))
        }
        else {
            library.add_type(INTERNAL_NAMESPACE, &format!("[#{:?}]", inner), Type::CArray(inner))
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

    pub fn function(library: &mut Library, func: Function) -> TypeId {
        let mut param_tids: Vec<TypeId> = func.parameters.iter().map(|p| p.typ).collect();
        param_tids.push(func.ret.typ);
        let typ = Type::Function(func);
        library.add_type(INTERNAL_NAMESPACE, &format!("fn<#{:?}>", param_tids), typ)
    }

    pub fn union(library: &mut Library, fields: Vec<Field>) -> TypeId {
        let field_tids: Vec<TypeId> = fields.iter().map(|f| f.typ).collect();
        let typ = Type::Union(Union { fields: fields, .. Union::default() });
        library.add_type(INTERNAL_NAMESPACE, &format!("#{:?}", field_tids), typ)
    }
}

macro_rules! impl_maybe_ref {
    () => ();
    ($name:ident, $($more:ident,)*) => (
        impl_maybe_ref!($($more,)*);

        impl MaybeRef<$name> for Type {
            fn maybe_ref(&self) -> Option<&$name> {
                if let &Type::$name(ref x) = self { Some(x) } else { None }
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
    fn maybe_ref_as<T>(&self) -> Option<&T> where Self: MaybeRef<T> {
        self.maybe_ref()
    }

    fn to_ref_as<T>(&self) -> &T where Self: MaybeRef<T> {
        self.to_ref()
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

    fn type_mut(&mut self, id: u32) -> &mut Type {
        self.types[id as usize].as_mut().unwrap()
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
pub const MAIN_NAMESPACE: u16 = 1;

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
        assert!(library.add_namespace(INTERNAL_NAMESPACE_NAME) == INTERNAL_NAMESPACE);
        for &(name, t) in &FUNDAMENTAL {
            library.add_type(INTERNAL_NAMESPACE, name, Type::Fundamental(t));
        }
        assert!(library.add_namespace(main_namespace_name) == MAIN_NAMESPACE);
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
        let (mut ns, name) = split_namespace_name(name);
        if name == "GType" {
            ns = None;
        }

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

    pub fn find_type_unwrapped(&self, current_ns_id: u16, name: &str, kind: &str) -> TypeId {
        self.find_type(current_ns_id, name)
            .unwrap_or_else(|| panic!("{} {} not found.", kind, name))
    }

    pub fn find_or_stub_type(&mut self, current_ns_id: u16, name: &str) -> TypeId {
        if let Some(tid) = self.find_type(current_ns_id, name) {
            return tid;
        }

        let (ns, name) = split_namespace_name(name);

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

    pub fn type_mut(&mut self, tid: TypeId) -> &mut Type {
        self.namespace_mut(tid.ns_id).type_mut(tid.id)
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

    pub fn fill_in(&mut self) {
        self.fix_gtype();
        self.check_resolved();
        self.fill_class_relationships();
        self.fill_class_iface_relationships();
    }

    pub fn fix_gtype(&mut self) {
        if let Some(ns_id) = self.find_namespace("GObject") {
            // hide the `GType` type alias in `GObject`
            self.add_type(ns_id, "Type", Type::Fundamental(Fundamental::Unsupported));
        }
    }

    fn fill_class_relationships(&mut self) {
        let mut classes = Vec::new();
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for id in 0..ns.types.len() {
                let tid = TypeId { ns_id: ns_id as u16, id: id as u32 };
                if let Type::Class(_) = *self.type_(tid) {
                    classes.push(tid);
                }
            }
        }

        let mut parents = Vec::with_capacity(10);
        for tid in classes {
            parents.clear();

            let mut first_parent_tid: Option<TypeId> = None;
            if let Type::Class(ref klass) = *self.type_(tid) {
                let mut parent = klass.parent;
                if let Some(parent_tid) = parent {
                    first_parent_tid = Some(parent_tid);
                }
                while let Some(parent_tid) = parent {
                    parents.push(parent_tid);
                    parent = self.type_(parent_tid).to_ref_as::<Class>().parent;
                }
            }

            if let Type::Class(ref mut klass) = *self.type_mut(tid) {
                parents.iter().map(|&tid| klass.parents.push(tid)).count();
            }

            if let Some(parent_tid) = first_parent_tid {
                if let Type::Class(ref mut klass) = *self.type_mut(parent_tid) {
                    klass.children.insert(tid);
                }
            }
        }
    }

    fn fill_class_iface_relationships(&mut self) {
        let mut ifaces = Vec::new();
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for id in 0..ns.types.len() {
                let tid = TypeId { ns_id: ns_id as u16, id: id as u32 };
                if let Type::Interface(_) = *self.type_(tid) {
                    ifaces.push(tid);
                }
            }
        }

        let mut prereqs = Vec::with_capacity(10);
        for tid in ifaces {
            prereqs.clear();
            if let Type::Interface(ref iface) = *self.type_(tid) {
                for &c_tid in &iface.prerequisites {
                    if let Type::Class(ref klass) = *self.type_(c_tid) {
                        prereqs.push(c_tid);
                        klass.parents.iter().map(|&p_tid| prereqs.push(p_tid)).count();
                    }
                }
            }
            prereqs.sort();
            prereqs.dedup();
            if let Type::Interface(ref mut iface) = *self.type_mut(tid) {
                prereqs.iter().map(|&tid| iface.prereq_parents.push(tid)).count();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use traits::*;

    fn make_library() -> Library {
        let mut lib = Library::new("Gtk");
        let glib_ns_id = lib.add_namespace("GLib");
        let gtk_ns_id = lib.add_namespace("Gtk");
        let object_tid = lib.add_type(glib_ns_id, "Object".into(), Type::Class(
            Class {
                name: "Object".into(),
                c_type: "GObject".into(),
                glib_get_type: "g_object_get_type".into(),
                .. Class::default()
            }));
        let ioobject_tid = lib.add_type(glib_ns_id, "InitiallyUnowned".into(), Type::Class(
            Class {
                name: "InitiallyUnowned".into(),
                c_type: "GInitiallyUnowned".into(),
                glib_get_type: "g_initially_unowned_get_type".into(),
                parent: Some(object_tid),
                .. Class::default()
            }));
        lib.add_type(gtk_ns_id, "Widget".into(), Type::Class(
            Class {
                name: "Widget".into(),
                c_type: "GtkWidget".into(),
                glib_get_type: "gtk_widget_get_type".into(),
                parent: Some(ioobject_tid),
                .. Class::default()
            }));
        lib
    }

    #[test]
    fn fill_class_parents() {
        let mut lib = make_library();
        lib.fill_in();
        let object_tid = lib.find_type(0, "GLib.Object").unwrap();
        let ioobject_tid = lib.find_type(0, "GLib.InitiallyUnowned").unwrap();
        let widget_tid = lib.find_type(0, "Gtk.Widget").unwrap();
        assert_eq!(lib.type_(object_tid).to_ref_as::<Class>().parents, &[]);
        assert_eq!(lib.type_(ioobject_tid).to_ref_as::<Class>().parents, &[object_tid]);
        assert_eq!(lib.type_(widget_tid).to_ref_as::<Class>().parents, &[ioobject_tid, object_tid]);
    }
}
