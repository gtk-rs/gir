use std::sync::Arc;

use crate::{env, library::*};

#[derive(Default, Clone, Debug, Eq, PartialEq)]
pub enum ConversionType {
    /// Coded without conversion.
    Direct,
    /// Coded with from_glib.
    Scalar,
    /// Type implementing TryFromGlib<Error=GlibNoneError>.
    Option,
    /// Type implementing TryFromGlib<Err> where Err is neither GlibNoneError
    /// nor GlibNoneOrInvalidError. Embeds the Error type name.
    /// Defaults to the object's type for the `Ok` variant if `ok_type` is
    /// `None`.
    Result {
        ok_type: Arc<str>,
        err_type: Arc<str>,
    },
    /// Coded with from_glib_xxx.
    Pointer,
    // Same as Pointer, except that use from_glib_borrow instead from_glib_none.
    Borrow,
    #[default]
    Unknown,
}

impl ConversionType {
    pub fn of(env: &env::Env, type_id: TypeId) -> Self {
        use crate::library::{Basic::*, Type::*};

        let library = &env.library;

        if let Some(conversion_type) = env
            .config
            .objects
            .get(&type_id.full_name(library))
            .and_then(|gobject| gobject.conversion_type.clone())
        {
            return conversion_type;
        }

        match library.type_(type_id) {
            Basic(fund) => match fund {
                Boolean => Self::Scalar,
                Int8 => Self::Direct,
                UInt8 => Self::Direct,
                Int16 => Self::Direct,
                UInt16 => Self::Direct,
                Int32 => Self::Direct,
                UInt32 => Self::Direct,
                Int64 => Self::Direct,
                UInt64 => Self::Direct,
                Char => Self::Scalar,
                UChar => Self::Scalar,
                Short => Self::Direct,
                UShort => Self::Direct,
                Int => Self::Direct,
                UInt => Self::Direct,
                Long => Self::Direct,
                ULong => Self::Direct,
                Size => Self::Direct,
                SSize => Self::Direct,
                Float => Self::Direct,
                Double => Self::Direct,
                UniChar => Self::Scalar,
                Pointer => Self::Pointer,
                VarArgs => Self::Unknown,
                Utf8 => Self::Pointer,
                Filename => Self::Pointer,
                OsString => Self::Pointer,
                Type => Self::Scalar,
                None => Self::Unknown,
                IntPtr => Self::Direct,
                UIntPtr => Self::Direct,
                Bool => Self::Direct,
                Unsupported => Self::Unknown,
            },
            Alias(alias) if alias.c_identifier == "GQuark" => Self::Scalar,
            Alias(alias) => Self::of(env, alias.typ),
            Bitfield(_) => Self::Scalar,
            Record(_) => Self::Pointer,
            Union(_) => Self::Pointer,
            Enumeration(_) => Self::Scalar,
            Interface(_) => Self::Pointer,
            Class(_) => Self::Pointer,
            CArray(_) => Self::Pointer,
            FixedArray(..) => Self::Pointer,
            List(_) => Self::Pointer,
            SList(_) => Self::Pointer,
            PtrArray(_) => Self::Pointer,
            Function(super::library::Function { name, .. }) if name == "AsyncReadyCallback" => {
                Self::Direct
            }
            Function(_) => Self::Direct,
            Custom(super::library::Custom {
                conversion_type, ..
            }) => conversion_type.clone(),
            _ => Self::Unknown,
        }
    }

    pub fn can_use_to_generate(&self) -> bool {
        matches!(self, Self::Option | Self::Result { .. })
    }
}
