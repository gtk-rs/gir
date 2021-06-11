use super::conversion_type::ConversionType;
use crate::{
    analysis::{ref_mode::RefMode, try_from_glib::TryFromGlib},
    env::Env,
    library::{self, Nullable, ParameterDirection, ParameterScope},
    nameutil::{is_gstring, use_glib_type},
    traits::*,
};

use std::{borrow::Borrow, result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeError {
    Ignored(String),
    Mismatch(String),
    Unimplemented(String),
}

/// A `RustType` definition and its associated types to be `use`d.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RustType {
    inner: String,
    used_types: Vec<String>,
}

impl RustType {
    /// Try building the `RustType` with no specific additional configuration.
    pub fn try_new(env: &Env, type_id: library::TypeId) -> Result {
        RustTypeBuilder::new(env, type_id).try_build()
    }

    /// Create a `RustTypeBuilder` which allows specifying additional configuration.
    pub fn builder(env: &Env, type_id: library::TypeId) -> RustTypeBuilder<'_> {
        RustTypeBuilder::new(env, type_id)
    }

    fn new_and_use(rust_type: impl ToString) -> Self {
        RustType {
            inner: rust_type.to_string(),
            used_types: vec![rust_type.to_string()],
        }
    }

    fn new_with_uses(rust_type: impl ToString, uses: &[impl ToString]) -> Self {
        RustType {
            inner: rust_type.to_string(),
            used_types: uses.iter().map(ToString::to_string).collect(),
        }
    }

    fn check(
        env: &Env,
        type_id: library::TypeId,
        type_name: impl ToString,
    ) -> result::Result<String, TypeError> {
        let mut type_name = type_name.to_string();

        if type_id.ns_id != library::MAIN_NAMESPACE
            && type_id.ns_id != library::INTERNAL_NAMESPACE
            && type_id.full_name(&env.library) != "GLib.DestroyNotify"
            && type_id.full_name(&env.library) != "GObject.Callback"
        {
            type_name = format!(
                "{}::{}",
                env.namespaces[type_id.ns_id].higher_crate_name,
                type_name.as_str()
            );

            if env.type_status(&type_id.full_name(&env.library)).ignored() {
                return Err(TypeError::Ignored(type_name));
            }
        }

        Ok(type_name)
    }

    fn try_new_and_use(env: &Env, type_id: library::TypeId) -> Result {
        Self::check(env, type_id, env.library.type_(type_id).get_name()).map(|type_name| RustType {
            inner: type_name.clone(),
            used_types: vec![type_name],
        })
    }

    fn try_new_and_use_with_name(
        env: &Env,
        type_id: library::TypeId,
        type_name: impl ToString,
    ) -> Result {
        Self::check(env, type_id, type_name).map(|type_name| RustType {
            inner: type_name.clone(),
            used_types: vec![type_name],
        })
    }

    pub fn used_types(&self) -> &Vec<String> {
        &self.used_types
    }

    pub fn into_used_types(self) -> Vec<String> {
        self.used_types
    }

    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    #[inline]
    pub fn alter_type(mut self, op: impl FnOnce(String) -> String) -> Self {
        self.inner = op(self.inner);
        self
    }

    #[inline]
    fn format_parameter(self, direction: ParameterDirection) -> Self {
        if direction.is_out() {
            self.alter_type(|type_| format!("&mut {}", type_))
        } else {
            self
        }
    }

    #[inline]
    fn apply_ref_mode(self, ref_mode: RefMode) -> Self {
        match ref_mode.for_rust_type() {
            "" => self,
            ref_mode => self.alter_type(|typ_| format!("{}{}", ref_mode, typ_)),
        }
    }
}

impl<T: ToString> From<T> for RustType {
    fn from(rust_type: T) -> Self {
        RustType {
            inner: rust_type.to_string(),
            used_types: Vec::new(),
        }
    }
}

impl IntoString for RustType {
    fn into_string(self) -> String {
        self.inner
    }
}

pub type Result = result::Result<RustType, TypeError>;

fn into_inner(res: Result) -> String {
    use self::TypeError::*;
    match res {
        Ok(rust_type) => rust_type.into_string(),
        Err(Ignored(s)) | Err(Mismatch(s)) | Err(Unimplemented(s)) => s,
    }
}

impl IntoString for Result {
    fn into_string(self) -> String {
        use self::TypeError::*;
        match self {
            Ok(s) => s.into_string(),
            Err(Ignored(s)) => format!("/*Ignored*/{}", s),
            Err(Mismatch(s)) => format!("/*Metadata mismatch*/{}", s),
            Err(Unimplemented(s)) => format!("/*Unimplemented*/{}", s),
        }
    }
}

impl MapAny<RustType> for Result {
    fn map_any<F: FnOnce(RustType) -> RustType>(self, op: F) -> Result {
        use self::TypeError::*;
        match self {
            Ok(rust_type) => Ok(op(rust_type)),
            Err(Ignored(s)) => Err(Ignored(op(s.into()).into_string())),
            Err(Mismatch(s)) => Err(Mismatch(op(s.into()).into_string())),
            Err(Unimplemented(s)) => Err(Unimplemented(op(s.into()).into_string())),
        }
    }
}

pub struct RustTypeBuilder<'env> {
    env: &'env Env,
    type_id: library::TypeId,
    direction: ParameterDirection,
    nullable: Nullable,
    ref_mode: RefMode,
    scope: ParameterScope,
    concurrency: library::Concurrency,
    try_from_glib: TryFromGlib,
}

impl<'env> RustTypeBuilder<'env> {
    fn new(env: &'env Env, type_id: library::TypeId) -> Self {
        RustTypeBuilder {
            env,
            type_id,
            direction: ParameterDirection::None,
            nullable: Nullable(false),
            ref_mode: RefMode::None,
            scope: ParameterScope::None,
            concurrency: library::Concurrency::None,
            try_from_glib: TryFromGlib::default(),
        }
    }

    pub fn direction(mut self, direction: ParameterDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn nullable(mut self, nullable: Nullable) -> Self {
        self.nullable = nullable;
        self
    }

    pub fn ref_mode(mut self, ref_mode: RefMode) -> Self {
        self.ref_mode = ref_mode;
        self
    }

    pub fn scope(mut self, scope: ParameterScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn concurrency(mut self, concurrency: library::Concurrency) -> Self {
        self.concurrency = concurrency;
        self
    }

    pub fn try_from_glib(mut self, try_from_glib: &TryFromGlib) -> Self {
        self.try_from_glib = try_from_glib.clone();
        self
    }

    pub fn try_build(self) -> Result {
        use crate::library::{Fundamental::*, Type::*};
        let ok = |s: &str| Ok(RustType::from(s));
        let ok_and_use = |s: &str| Ok(RustType::new_and_use(s));
        let err = |s: &str| Err(TypeError::Unimplemented(s.into()));
        let mut skip_option = false;
        let mut skip_ref = false;
        let type_ = self.env.library.type_(self.type_id);
        let mut rust_type = match *type_ {
            Fundamental(fund) => {
                match fund {
                    None => err("()"),
                    Boolean => ok("bool"),
                    Int8 => ok("i8"),
                    UInt8 => ok("u8"),
                    Int16 => ok("i16"),
                    UInt16 => ok("u16"),
                    Int32 => ok("i32"),
                    UInt32 => ok("u32"),
                    Int64 => ok("i64"),
                    UInt64 => ok("u64"),

                    Int => ok("i32"),  //maybe dependent on target system
                    UInt => ok("u32"), //maybe dependent on target system

                    Short => ok_and_use("libc::c_short"), //depends of target system
                    UShort => ok_and_use("libc::c_ushort"), //depends o f target system
                    Long => ok_and_use("libc::c_long"),   //depends of target system
                    ULong => ok_and_use("libc::c_ulong"), //depends of target system

                    Size => ok("usize"),  //depends of target system
                    SSize => ok("isize"), //depends of target system

                    Float => ok("f32"),
                    Double => ok("f64"),

                    UniChar => ok("char"),
                    Utf8 => {
                        if self.ref_mode.is_ref() {
                            skip_ref = true;
                            ok_and_use(&use_glib_type(&self.env, "GString"))
                        } else {
                            ok_and_use(&use_glib_type(&self.env, "GString"))
                        }
                    }
                    Filename => {
                        if self.ref_mode.is_ref() {
                            ok_and_use("std::path::Path")
                        } else {
                            ok_and_use("std::path::PathBuf")
                        }
                    }
                    OsString => {
                        if self.ref_mode.is_ref() {
                            ok_and_use("std::ffi::OsStr")
                        } else {
                            ok_and_use("std::ffi::OsString")
                        }
                    }
                    Type => ok_and_use(&use_glib_type(self.env, "types::Type")),
                    Char => ok_and_use(&use_glib_type(self.env, "Char")),
                    UChar => ok_and_use(&use_glib_type(self.env, "UChar")),
                    Unsupported => err("Unsupported"),
                    _ => err(&format!("Fundamental: {:?}", fund)),
                }
            }
            Alias(ref alias) => {
                RustType::try_new_and_use(self.env, self.type_id).and_then(|alias_rust_type| {
                    RustType::builder(&self.env, alias.typ)
                        .direction(self.direction)
                        .nullable(self.nullable)
                        .ref_mode(self.ref_mode)
                        .scope(self.scope)
                        .concurrency(self.concurrency)
                        .try_from_glib(&self.try_from_glib)
                        .try_build()
                        .map_any(|_| alias_rust_type)
                })
            }
            Record(library::Record { ref c_type, .. }) if c_type == "GVariantType" => {
                let type_name = if self.ref_mode.is_ref() {
                    "VariantTy"
                } else {
                    "VariantType"
                };
                RustType::try_new_and_use_with_name(self.env, self.type_id, type_name)
            }
            Enumeration(..) | Bitfield(..) | Record(..) | Union(..) | Class(..) | Interface(..) => {
                RustType::try_new_and_use(self.env, self.type_id).and_then(|rust_type| {
                    if self
                        .env
                        .type_status(&self.type_id.full_name(&self.env.library))
                        .ignored()
                    {
                        Err(TypeError::Ignored(rust_type.into_string()))
                    } else {
                        Ok(rust_type)
                    }
                })
            }
            List(inner_tid) | SList(inner_tid) | CArray(inner_tid) | PtrArray(inner_tid)
                if ConversionType::of(self.env, inner_tid) == ConversionType::Pointer =>
            {
                skip_option = true;
                let inner_ref_mode = match self.env.library.type_(inner_tid) {
                    Class(..) | Interface(..) => RefMode::None,
                    _ => self.ref_mode,
                };
                RustType::builder(&self.env, inner_tid)
                    .ref_mode(inner_ref_mode)
                    .scope(self.scope)
                    .concurrency(self.concurrency)
                    .try_build()
                    .map_any(|rust_type| {
                        rust_type.alter_type(|typ| {
                            if self.ref_mode.is_ref() {
                                format!("[{}]", typ)
                            } else {
                                format!("Vec<{}>", typ)
                            }
                        })
                    })
            }
            CArray(inner_tid)
                if ConversionType::of(self.env, inner_tid) == ConversionType::Direct =>
            {
                if let Fundamental(fund) = self.env.library.type_(inner_tid) {
                    let array_type = match fund {
                        Int8 => Some("i8"),
                        UInt8 => Some("u8"),
                        Int16 => Some("i16"),
                        UInt16 => Some("u16"),
                        Int32 => Some("i32"),
                        UInt32 => Some("u32"),
                        Int64 => Some("i64"),
                        UInt64 => Some("u64"),

                        Int => Some("i32"),  //maybe dependent on target system
                        UInt => Some("u32"), //maybe dependent on target system

                        Float => Some("f32"),
                        Double => Some("f64"),
                        _ => Option::None,
                    };

                    if let Some(s) = array_type {
                        skip_option = true;
                        if self.ref_mode.is_ref() {
                            Ok(format!("[{}]", s).into())
                        } else {
                            Ok(format!("Vec<{}>", s).into())
                        }
                    } else {
                        Err(TypeError::Unimplemented(type_.get_name()))
                    }
                } else {
                    Err(TypeError::Unimplemented(type_.get_name()))
                }
            }
            Custom(library::Custom { ref name, .. }) => {
                RustType::try_new_and_use_with_name(&self.env, self.type_id, name)
            }
            Function(ref f) => {
                let concurrency = match self.concurrency {
                    _ if self.scope.is_call() => "",
                    library::Concurrency::Send | library::Concurrency::SendUnique => " + Send",
                    // If an object is Sync, it can be shared between threads, and as
                    // such our callback can be called from arbitrary threads and needs
                    // to be Send *AND* Sync
                    library::Concurrency::SendSync => " + Send + Sync",
                    library::Concurrency::None => "",
                };

                let full_name = self.type_id.full_name(&self.env.library);
                if full_name == "Gio.AsyncReadyCallback" {
                    // FIXME need to use the result from use_glib_type(&self.env, "Error")?
                    return Ok(format!(
                        "FnOnce(Result<(), {}>) + 'static",
                        use_glib_type(&self.env, "Error"),
                    )
                    .into());
                } else if full_name == "GLib.DestroyNotify" {
                    return Ok(format!("Fn(){} + 'static", concurrency).into());
                }
                let mut params = Vec::with_capacity(f.parameters.len());
                let mut err = false;
                for p in f.parameters.iter() {
                    if p.closure.is_some() {
                        continue;
                    }

                    let p_res = RustType::builder(&self.env, p.typ)
                        .direction(p.direction)
                        .nullable(p.nullable)
                        .try_build();
                    match p_res {
                        Ok(p_rust_type) => {
                            let is_fundamental = p.typ.is_fundamental_type(&self.env);
                            let y = RustType::try_new(&self.env, p.typ)
                                .unwrap_or_else(|_| RustType::default());
                            params.push(format!(
                                "{}{}",
                                if is_fundamental || *p.nullable {
                                    ""
                                } else {
                                    "&"
                                },
                                if !is_gstring(y.as_str()) {
                                    if !is_fundamental && *p.nullable {
                                        p_rust_type.into_string().replace("Option<", "Option<&")
                                    } else {
                                        p_rust_type.into_string()
                                    }
                                } else if *p.nullable {
                                    "Option<&str>".to_owned()
                                } else {
                                    "&str".to_owned()
                                }
                            ));
                        }
                        e => {
                            err = true;
                            params.push(e.into_string());
                        }
                    }
                }
                let closure_kind = if self.scope.is_call() {
                    "FnMut"
                } else if self.scope.is_async() {
                    "FnOnce"
                } else {
                    "Fn"
                };
                let ret_res = RustType::builder(&self.env, f.ret.typ)
                    .direction(f.ret.direction)
                    .nullable(f.ret.nullable)
                    .try_build();
                let ret = match ret_res {
                    Ok(ret_rust_type) => {
                        let y = RustType::try_new(&self.env, f.ret.typ)
                            .unwrap_or_else(|_| RustType::default());
                        format!(
                            "{}({}) -> {}{}",
                            closure_kind,
                            params.join(", "),
                            if !is_gstring(&y.as_str()) {
                                ret_rust_type.as_str()
                            } else if *f.ret.nullable {
                                "Option<String>"
                            } else {
                                "String"
                            },
                            concurrency
                        )
                    }
                    Err(TypeError::Unimplemented(ref x)) if x == "()" => {
                        format!("{}({}){}", closure_kind, params.join(", "), concurrency)
                    }
                    e => {
                        err = true;
                        format!(
                            "{}({}) -> {}{}",
                            closure_kind,
                            params.join(", "),
                            e.into_string(),
                            concurrency
                        )
                    }
                };
                if err {
                    return Err(TypeError::Unimplemented(ret));
                }
                Ok(if *self.nullable {
                    if self.scope.is_call() {
                        format!("Option<&mut dyn ({})>", ret)
                    } else {
                        format!("Option<Box_<dyn {} + 'static>>", ret)
                    }
                } else {
                    format!(
                        "{}{}",
                        ret,
                        if self.scope.is_call() {
                            ""
                        } else {
                            " + 'static"
                        }
                    )
                }
                .into())
            }
            _ => Err(TypeError::Unimplemented(type_.get_name())),
        };

        match self
            .try_from_glib
            .or_type_defaults(&self.env, self.type_id)
            .borrow()
        {
            TryFromGlib::Option => {
                rust_type = rust_type.map_any(|rust_type| {
                    rust_type
                        .alter_type(|typ_| {
                            let mut opt = format!("Option<{}>", typ_);
                            if self.direction == ParameterDirection::In {
                                opt = format!("impl Into<{}>", opt);
                            }

                            opt
                        })
                        .apply_ref_mode(if skip_ref {
                            RefMode::None
                        } else {
                            self.ref_mode
                        })
                });
            }
            TryFromGlib::Result { ok_type, err_type } => {
                if self.direction == ParameterDirection::In {
                    rust_type = rust_type.map_any(|rust_type| {
                        RustType::new_with_uses(
                            format!("impl Into<{}>", &rust_type.as_str()),
                            &[&rust_type.as_str()],
                        )
                    });
                } else {
                    rust_type = rust_type.map_any(|_| {
                        RustType::new_with_uses(
                            format!("Result<{}, {}>", &ok_type, &err_type),
                            &[ok_type, err_type],
                        )
                    });
                }
            }
            TryFromGlib::ResultInfallible { ok_type } => {
                let new_rust_type = RustType::new_and_use(ok_type).apply_ref_mode(if skip_ref {
                    RefMode::None
                } else {
                    self.ref_mode
                });
                rust_type = rust_type.map_any(|_| new_rust_type);
            }
            _ => {
                rust_type = rust_type.map_any(|rust_type| {
                    rust_type.apply_ref_mode(if skip_ref {
                        RefMode::None
                    } else {
                        self.ref_mode
                    })
                });
            }
        }

        if *self.nullable && !skip_option {
            match ConversionType::of(&self.env, self.type_id) {
                ConversionType::Pointer | ConversionType::Scalar => {
                    rust_type = rust_type.map_any(|rust_type| {
                        rust_type.alter_type(|typ_| format!("Option<{}>", typ_))
                    });
                }
                _ => (),
            }
        }
        rust_type
    }

    pub fn try_build_param(self) -> Result {
        use crate::library::Type::*;
        let type_ = self.env.library.type_(self.type_id);

        if self.direction == ParameterDirection::None {
            panic!("undefined direction for parameter with type {:?}", type_);
        }

        let rust_type = RustType::builder(&self.env, self.type_id)
            .direction(self.direction)
            .nullable(self.nullable)
            .ref_mode(self.ref_mode)
            .scope(self.scope)
            .try_from_glib(&self.try_from_glib)
            .try_build();
        match type_ {
            Fundamental(library::Fundamental::Utf8)
            | Fundamental(library::Fundamental::OsString)
            | Fundamental(library::Fundamental::Filename)
                if (self.direction == ParameterDirection::InOut
                    || (self.direction == ParameterDirection::Out
                        && self.ref_mode == RefMode::ByRefMut)) =>
            {
                Err(TypeError::Unimplemented(into_inner(rust_type)))
            }
            Fundamental(_) => {
                rust_type.map_any(|rust_type| rust_type.format_parameter(self.direction))
            }

            Alias(alias) => rust_type
                .and_then(|rust_type| {
                    RustType::builder(&self.env, alias.typ)
                        .direction(self.direction)
                        .nullable(self.nullable)
                        .ref_mode(self.ref_mode)
                        .scope(self.scope)
                        .try_from_glib(&self.try_from_glib)
                        .try_build_param()
                        .map_any(|_| rust_type)
                })
                .map_any(|rust_type| rust_type.format_parameter(self.direction)),

            Enumeration(..) | Union(..) | Bitfield(..) => {
                rust_type.map_any(|rust_type| rust_type.format_parameter(self.direction))
            }

            Record(..) => {
                if self.direction == ParameterDirection::InOut {
                    Err(TypeError::Unimplemented(into_inner(rust_type)))
                } else {
                    rust_type
                }
            }

            Class(..) | Interface(..) => match self.direction {
                ParameterDirection::In | ParameterDirection::Out | ParameterDirection::Return => {
                    rust_type
                }
                _ => Err(TypeError::Unimplemented(into_inner(rust_type))),
            },

            List(..) | SList(..) => match self.direction {
                ParameterDirection::In | ParameterDirection::Return => rust_type,
                _ => Err(TypeError::Unimplemented(into_inner(rust_type))),
            },
            CArray(..) | PtrArray(..) => match self.direction {
                ParameterDirection::In | ParameterDirection::Out | ParameterDirection::Return => {
                    rust_type.map_any(|rust_type| rust_type.format_parameter(self.direction))
                }
                _ => Err(TypeError::Unimplemented(into_inner(rust_type))),
            },
            Function(ref func) if func.name == "AsyncReadyCallback" => {
                Ok("AsyncReadyCallback".into())
            }
            Function(_) => rust_type,
            Custom(..) => rust_type.map(|rust_type| rust_type.format_parameter(self.direction)),
            _ => Err(TypeError::Unimplemented(type_.get_name())),
        }
    }
}
