use std::{borrow::Borrow, fmt, result};

use super::conversion_type::ConversionType;
use crate::{
    analysis::{record_type::RecordType, ref_mode::RefMode, try_from_glib::TryFromGlib},
    config::functions::{CallbackParameter, CallbackParameters},
    env::Env,
    library::{self, Nullable, ParameterDirection, ParameterScope},
    nameutil::{is_gstring, use_glib_type},
    traits::*,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeError {
    Ignored(RustType),
    Mismatch(RustType),
    Unimplemented(RustType),
    UnknownConversion(RustType),
}

impl TypeError {
    pub fn ignored(r: impl Into<RustType>) -> Self {
        Self::Ignored(r.into())
    }
    pub fn mismatch(r: impl Into<RustType>) -> Self {
        Self::Mismatch(r.into())
    }
    pub fn unimplemented(r: impl Into<RustType>) -> Self {
        Self::Unimplemented(r.into())
    }
    pub fn message(&self) -> &'static str {
        match self {
            Self::Ignored(_) => "Ignored",
            Self::Mismatch(_) => "Mismatch",
            Self::Unimplemented(_) => "Unimplemented",
            Self::UnknownConversion(_) => "Unknown conversion",
        }
    }
}

/// A `RustType` definition and its associated types to be `use`d.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RustType {
    inner: String,
    used_types: Vec<String>,
    nullable_as_option: bool,
}

impl RustType {
    /// Try building the `RustType` with no specific additional configuration.
    pub fn try_new(env: &Env, type_id: library::TypeId) -> Result {
        RustTypeBuilder::new(env, type_id).try_build()
    }

    /// Create a `RustTypeBuilder` which allows specifying additional
    /// configuration.
    pub fn builder(env: &Env, type_id: library::TypeId) -> RustTypeBuilder<'_> {
        RustTypeBuilder::new(env, type_id)
    }

    fn new_and_use(rust_type: &impl ToString) -> Self {
        RustType {
            inner: rust_type.to_string(),
            used_types: vec![rust_type.to_string()],
            nullable_as_option: true,
        }
    }

    fn check(
        env: &Env,
        type_id: library::TypeId,
        type_name: &impl ToString,
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
                return Err(TypeError::ignored(type_name));
            }
        }

        Ok(type_name)
    }

    fn try_new_and_use(env: &Env, type_id: library::TypeId) -> Result {
        Self::check(env, type_id, &env.library.type_(type_id).get_name()).map(|type_name| {
            RustType {
                inner: type_name.clone(),
                used_types: vec![type_name],
                nullable_as_option: true,
            }
        })
    }

    fn try_new_and_use_with_name(
        env: &Env,
        type_id: library::TypeId,
        type_name: impl ToString,
    ) -> Result {
        Self::check(env, type_id, &type_name).map(|type_name| RustType {
            inner: type_name.clone(),
            used_types: vec![type_name],
            nullable_as_option: true,
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

    pub fn alter_type(mut self, op: impl FnOnce(String) -> String) -> Self {
        self.inner = op(self.inner);
        self
    }
}

impl fmt::Display for RustType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}

impl<T: AsRef<str>> From<T> for RustType {
    fn from(rust_type: T) -> Self {
        RustType {
            inner: rust_type.as_ref().to_string(),
            used_types: Vec::new(),
            nullable_as_option: true,
        }
    }
}

impl IntoString for RustType {
    fn into_string(self) -> String {
        self.inner
    }
}

pub type Result = result::Result<RustType, TypeError>;

fn unwrap_rust_type(res: Result) -> RustType {
    use self::TypeError::*;
    match res {
        Ok(r) => r,
        Err(Ignored(r) | Mismatch(r) | Unimplemented(r) | UnknownConversion(r)) => r,
    }
}

impl IntoString for Result {
    fn into_string(self) -> String {
        use self::TypeError::*;
        match self {
            Ok(r) => r.into_string(),
            Err(ref err) => match err {
                Ignored(r) | Mismatch(r) | Unimplemented(r) | UnknownConversion(r) => {
                    format!("/*{}*/{r}", err.message())
                }
            },
        }
    }
}

impl MapAny<RustType> for Result {
    fn map_any<F: FnOnce(RustType) -> RustType>(self, op: F) -> Result {
        use self::TypeError::*;
        match self {
            Ok(r) => Ok(op(r)),
            Err(Ignored(r)) => Err(Ignored(op(r))),
            Err(Mismatch(r)) => Err(Mismatch(op(r))),
            Err(Unimplemented(r)) => Err(Unimplemented(op(r))),
            Err(UnknownConversion(r)) => Err(UnknownConversion(op(r))),
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
    callback_parameters_config: CallbackParameters,
    is_for_callback: bool,
}

impl<'env> RustTypeBuilder<'env> {
    fn new(env: &'env Env, type_id: library::TypeId) -> Self {
        Self {
            env,
            type_id,
            direction: ParameterDirection::None,
            nullable: Nullable(false),
            ref_mode: RefMode::None,
            scope: ParameterScope::None,
            concurrency: library::Concurrency::None,
            try_from_glib: TryFromGlib::default(),
            callback_parameters_config: Vec::new(),
            is_for_callback: false,
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

    pub fn callback_parameters_config(
        mut self,
        callback_parameters_config: &[CallbackParameter],
    ) -> Self {
        self.callback_parameters_config = callback_parameters_config.to_owned();
        self
    }

    pub fn for_callback(mut self, is_for_callback: bool) -> Self {
        self.is_for_callback = is_for_callback;
        self
    }

    pub fn try_build(self) -> Result {
        use crate::library::{Basic::*, Type::*};
        let ok = |s: &str| Ok(RustType::from(s));
        let ok_and_use = |s: &str| Ok(RustType::new_and_use(&s));
        let err = |s: &str| Err(TypeError::Unimplemented(s.into()));
        let type_ = self.env.library.type_(self.type_id);
        match *type_ {
            Basic(fund) => {
                match fund {
                    None => err("()"),
                    Boolean | Bool => ok("bool"),
                    Int8 => ok("i8"),
                    UInt8 => ok("u8"),
                    Int16 => ok("i16"),
                    UInt16 => ok("u16"),
                    Int32 => ok("i32"),
                    UInt32 => ok("u32"),
                    Int64 => ok("i64"),
                    UInt64 => ok("u64"),

                    Int => ok("i32"),  // maybe dependent on target system
                    UInt => ok("u32"), // maybe dependent on target system

                    Short => ok_and_use("libc::c_short"), // depends of target system
                    UShort => ok_and_use("libc::c_ushort"), // depends o f target system
                    Long => ok_and_use("libc::c_long"),   // depends of target system
                    ULong => ok_and_use("libc::c_ulong"), // depends of target system

                    TimeT => ok_and_use("libc::time_t"), // depends of target system
                    OffT => ok_and_use("libc::off_t"),   // depends of target system
                    DevT => ok_and_use("libc::dev_t"),   // depends of target system
                    GidT => ok_and_use("libc::gid_t"),   // depends of target system
                    PidT => ok_and_use("libc::pid_t"),   // depends of target system
                    SockLenT => ok_and_use("libc::socklen_t"), // depends of target system
                    UidT => ok_and_use("libc::uid_t"),   // depends of target system

                    Size => ok("usize"),  // depends of target system
                    SSize => ok("isize"), // depends of target system

                    Float => ok("f32"),
                    Double => ok("f64"),

                    UniChar => ok("char"),
                    Utf8 => {
                        if self.ref_mode.is_immutable() {
                            ok("str")
                        } else {
                            ok_and_use(&use_glib_type(self.env, "GString"))
                        }
                    }
                    Filename => {
                        if self.ref_mode.is_immutable() {
                            ok_and_use("std::path::Path")
                        } else {
                            ok_and_use("std::path::PathBuf")
                        }
                    }
                    OsString => {
                        if self.ref_mode.is_immutable() {
                            ok_and_use("std::ffi::OsStr")
                        } else {
                            ok_and_use("std::ffi::OsString")
                        }
                    }
                    Type => ok_and_use(&use_glib_type(self.env, "types::Type")),
                    Char => ok_and_use(&use_glib_type(self.env, "Char")),
                    UChar => ok_and_use(&use_glib_type(self.env, "UChar")),
                    Unsupported => err("Unsupported"),
                    _ => err(&format!("Basic: {fund:?}")),
                }
                .map_any(|mut r| {
                    r.nullable_as_option = !(
                        // passed to ffi as pointer to the stack allocated type
                        self.direction == ParameterDirection::Out
                            && ConversionType::of(self.env, self.type_id) == ConversionType::Direct
                    );
                    r
                })
            }
            Alias(ref alias) => {
                RustType::try_new_and_use(self.env, self.type_id).and_then(|mut r| {
                    RustType::builder(self.env, alias.typ)
                        .direction(self.direction)
                        .nullable(self.nullable)
                        .ref_mode(self.ref_mode)
                        .scope(self.scope)
                        .concurrency(self.concurrency)
                        .try_from_glib(&self.try_from_glib)
                        .callback_parameters_config(self.callback_parameters_config.as_ref())
                        .for_callback(self.is_for_callback)
                        .try_build()
                        .map_any(|alias_r| {
                            r.nullable_as_option = alias_r.nullable_as_option;
                            r
                        })
                })
            }
            Record(library::Record { ref c_type, .. }) if c_type == "GVariantType" => {
                let type_name = if self.ref_mode.is_immutable() {
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
                        Err(TypeError::ignored(rust_type))
                    } else {
                        Ok(rust_type)
                    }
                })
            }
            List(inner_tid) | SList(inner_tid) | CArray(inner_tid) | PtrArray(inner_tid)
                if ConversionType::of(self.env, inner_tid) == ConversionType::Pointer =>
            {
                let inner_ref_mode = match self.env.type_(inner_tid) {
                    Class(..) | Interface(..) => RefMode::None,
                    Record(record) => match RecordType::of(record) {
                        RecordType::Boxed => RefMode::None,
                        RecordType::AutoBoxed => {
                            if !record.has_copy() {
                                RefMode::None
                            } else {
                                self.ref_mode
                            }
                        }
                        _ => self.ref_mode,
                    },
                    _ => self.ref_mode,
                };
                RustType::builder(self.env, inner_tid)
                    .ref_mode(inner_ref_mode)
                    .scope(self.scope)
                    .concurrency(self.concurrency)
                    .try_build()
                    .map_any(|mut r| {
                        r.nullable_as_option = false; // null is empty
                        r.inner = if self.ref_mode.is_immutable() {
                            format!("[{}{}]", inner_ref_mode, r.inner)
                        } else {
                            format!("Vec<{}>", r.inner)
                        };
                        r
                    })
            }
            CArray(inner_tid)
                if ConversionType::of(self.env, inner_tid) == ConversionType::Direct =>
            {
                if let Basic(fund) = self.env.type_(inner_tid) {
                    let array_type = match fund {
                        Int8 => Some("i8"),
                        UInt8 => Some("u8"),
                        Int16 => Some("i16"),
                        UInt16 => Some("u16"),
                        Int32 => Some("i32"),
                        UInt32 => Some("u32"),
                        Int64 => Some("i64"),
                        UInt64 => Some("u64"),

                        Int => Some("i32"),  // maybe dependent on target system
                        UInt => Some("u32"), // maybe dependent on target system

                        Float => Some("f32"),
                        Double => Some("f64"),
                        _ => Option::None,
                    };

                    if let Some(s) = array_type {
                        if self.ref_mode.is_immutable() {
                            ok(&format!("[{s}]"))
                        } else {
                            ok(&format!("Vec<{s}>"))
                        }
                    } else {
                        Err(TypeError::unimplemented(type_.get_name()))
                    }
                } else {
                    Err(TypeError::unimplemented(type_.get_name()))
                }
                .map_any(|mut r| {
                    r.nullable_as_option = false; // null is empty
                    r
                })
            }
            Custom(library::Custom { ref name, .. }) => {
                RustType::try_new_and_use_with_name(self.env, self.type_id, name)
            }
            Function(ref f) => {
                let concurrency = match self.concurrency {
                    _ if self.scope.is_call() => "",
                    library::Concurrency::Send => " + Send",
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
                        use_glib_type(self.env, "Error"),
                    )
                    .into());
                } else if full_name == "GLib.DestroyNotify" {
                    return Ok(format!("Fn(){concurrency} + 'static").into());
                }

                let mut has_param_err = false;
                let mut params = Vec::with_capacity(f.parameters.len());
                for p in &f.parameters {
                    if p.closure.is_some() {
                        continue;
                    }
                    let nullable = self
                        .callback_parameters_config
                        .iter()
                        .find(|cp| cp.ident.is_match(&p.name))
                        .and_then(|c| c.nullable)
                        .unwrap_or(p.nullable);
                    let ref_mode = if p.typ.is_basic_type(self.env) {
                        RefMode::None
                    } else {
                        RefMode::ByRef
                    };
                    let param = RustType::builder(self.env, p.typ)
                        .direction(p.direction)
                        .nullable(nullable)
                        .ref_mode(ref_mode)
                        .for_callback(true)
                        .scope(self.scope)
                        .try_build_param();
                    has_param_err |= param.is_err();
                    params.push(param.into_string());
                }
                let params = params.join(", ");

                let closure_kind = if self.scope.is_call() {
                    "FnMut"
                } else if self.scope.is_async() {
                    "FnOnce"
                } else {
                    "Fn"
                };
                let res = if f.ret.c_type == "void" {
                    Ok(format!("{closure_kind}({params}){concurrency}").into())
                } else {
                    RustType::builder(self.env, f.ret.typ)
                        .direction(f.ret.direction)
                        .nullable(f.ret.nullable)
                        .for_callback(true)
                        .try_build_param()
                        .map_any(|mut r| {
                            r.inner = format!("{closure_kind}({params}) -> {r}{concurrency}");
                            r
                        })
                }
                .map_any(|mut func| {
                    // Handle nullability here as it affects the type e.g. for bounds
                    func.nullable_as_option = false;
                    match (*self.nullable, self.scope.is_call()) {
                        (false, true) => (),
                        (false, false) => func.inner = format!("{func} + 'static"),
                        (true, true) => func.inner = format!("Option<&mut dyn ({func})>"),
                        (true, false) => func.inner = format!("Option<Box_<dyn {func} + 'static>>"),
                    }
                    func
                });

                if has_param_err {
                    return Err(TypeError::unimplemented(unwrap_rust_type(res)));
                }

                res
            }
            _ => Err(TypeError::unimplemented(type_.get_name())),
        }
    }

    pub fn try_build_param(self) -> Result {
        use crate::library::Type::*;
        let type_ = self.env.library.type_(self.type_id);
        assert!(
            self.direction != ParameterDirection::None,
            "undefined direction for parameter with type {type_:?}"
        );
        let res = RustType::builder(self.env, self.type_id)
            .direction(self.direction)
            .nullable(self.nullable)
            .ref_mode(self.ref_mode)
            .scope(self.scope)
            .concurrency(self.concurrency)
            .try_from_glib(&self.try_from_glib)
            .callback_parameters_config(self.callback_parameters_config.as_ref())
            .for_callback(self.is_for_callback)
            .try_build();

        match type_ {
            Basic(_) | Enumeration(_) | Union(_) | Bitfield(_) | Custom(_) | Class(_)
            | Interface(_) | Record(_) | Function(_) | Alias(_) => (),
            CArray(_) | List(_) | SList(_) | PtrArray(_) => {
                if self.direction == ParameterDirection::InOut {
                    return Err(TypeError::unimplemented(unwrap_rust_type(res)));
                }
            }
            Array(..) | FixedArray(..) | HashTable(..) => {
                return Err(TypeError::unimplemented(unwrap_rust_type(res)));
            }
        }

        if let Ok(ref r) = res {
            if ConversionType::of(self.env, self.type_id) == ConversionType::Unknown {
                return Err(TypeError::UnknownConversion(r.clone()));
            }
            if self.direction == ParameterDirection::InOut && self.ref_mode.is_none() {
                return Err(TypeError::Ignored(r.clone()));
            }
        }

        res.map_any(|mut r| {
            let ref_ = self.ref_mode.to_string();
            match self
                .try_from_glib
                .or_type_defaults(self.env, self.type_id)
                .borrow()
            {
                TryFromGlib::Option => {
                    if self.direction == ParameterDirection::In && !self.is_for_callback {
                        r.inner = format!("impl Into<Option<{ref_}{r}>>");
                    } else {
                        r.inner = format!("Option<{r}>");
                    }
                }
                TryFromGlib::OptionMandatory => {
                    if self.direction == ParameterDirection::Return {
                        r.inner = r.inner.to_string();
                    } else {
                        r.inner = format!("{ref_}{r}");
                    }
                }
                TryFromGlib::Result { ok_type, err_type } => {
                    if self.direction == ParameterDirection::In && !self.is_for_callback {
                        r.used_types.push(r.inner.to_string());
                        r.inner = format!("impl Into<{}>", r.inner);
                    } else {
                        r.used_types.retain(|t| *t != r.inner);
                        r.used_types
                            .extend([ok_type, err_type].iter().map(ToString::to_string));
                        r.inner = format!("Result<{ok_type}, {err_type}>");
                    }
                }
                TryFromGlib::ResultInfallible { ok_type } => {
                    r.used_types.push(ok_type.to_string());
                    r.inner = ok_type.to_string();
                }
                _ if r.nullable_as_option && *self.nullable => {
                    r.inner = if self.direction == ParameterDirection::Return {
                        if self.is_for_callback && is_gstring(&r.inner) {
                            "Option<String>".to_string()
                        } else {
                            format!("Option<{r}>")
                        }
                    } else if self.is_for_callback && is_gstring(&r.inner) {
                        "Option<&str>".to_string()
                    } else {
                        format!("Option<{ref_}{r}>")
                    };
                }
                _ if self.is_for_callback && is_gstring(&r.inner) => {
                    r.inner = if self.direction == ParameterDirection::Return {
                        "String".to_string()
                    } else {
                        "&str".to_string()
                    };
                }
                _ => r.inner = format!("{ref_}{r}"),
            }

            r
        })
    }
}
