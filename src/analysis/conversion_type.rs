use crate::{env, library::*};

use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConversionType {
    /// Coded without conversion.
    Direct,
    /// Coded with from_glib.
    Scalar,
    /// Type implementing TryFromGlib<Error=GlibNoneError>.
    Option,
    /// Type implementing TryFromGlib<Err> where Err is neither GlibNoneError
    /// nor GlibNoneOrInvalidError. Embeds the Error type name.
    /// Defaults to the object's type for the `Ok` variant if `ok_type` is `None`.
    Result {
        ok_type: Arc<str>,
        err_type: Arc<str>,
    },
    /// Coded with from_glib_xxx.
    Pointer,
    // Same as Pointer, except that use from_glib_borrow instead from_glib_none.
    Borrow,
    Unknown,
}

impl Default for ConversionType {
    fn default() -> Self {
        ConversionType::Unknown
    }
}

impl ConversionType {
    pub fn of(env: &env::Env, type_id: TypeId) -> ConversionType {
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
                Boolean => ConversionType::Scalar,
                Int8 => ConversionType::Direct,
                UInt8 => ConversionType::Direct,
                Int16 => ConversionType::Direct,
                UInt16 => ConversionType::Direct,
                Int32 => ConversionType::Direct,
                UInt32 => ConversionType::Direct,
                Int64 => ConversionType::Direct,
                UInt64 => ConversionType::Direct,
                Char => ConversionType::Scalar,
                UChar => ConversionType::Scalar,
                Short => ConversionType::Direct,
                UShort => ConversionType::Direct,
                Int => ConversionType::Direct,
                UInt => ConversionType::Direct,
                Long => ConversionType::Direct,
                ULong => ConversionType::Direct,
                Size => ConversionType::Direct,
                SSize => ConversionType::Direct,
                Float => ConversionType::Direct,
                Double => ConversionType::Direct,
                UniChar => ConversionType::Scalar,
                Pointer => ConversionType::Pointer,
                VarArgs => ConversionType::Unknown,
                Utf8 => ConversionType::Pointer,
                Filename => ConversionType::Pointer,
                OsString => ConversionType::Pointer,
                Type => ConversionType::Scalar,
                None => ConversionType::Unknown,
                IntPtr => ConversionType::Direct,
                UIntPtr => ConversionType::Direct,
                Bool => ConversionType::Direct,
                Unsupported => ConversionType::Unknown,
            },
            Alias(alias) if alias.c_identifier == "GQuark" => ConversionType::Scalar,
            Alias(alias) => ConversionType::of(env, alias.typ),
            Bitfield(_) => ConversionType::Scalar,
            Record(_) => ConversionType::Pointer,
            Union(_) => ConversionType::Pointer,
            Enumeration(_) => ConversionType::Scalar,
            Interface(_) => ConversionType::Pointer,
            Class(_) => ConversionType::Pointer,
            CArray(_) => ConversionType::Pointer,
            FixedArray(..) => ConversionType::Pointer,
            List(_) => ConversionType::Pointer,
            SList(_) => ConversionType::Pointer,
            PtrArray(_) => ConversionType::Pointer,
            Function(super::library::Function { name, .. }) if name == "AsyncReadyCallback" => {
                ConversionType::Direct
            }
            Function(_) => ConversionType::Direct,
            Custom(super::library::Custom {
                conversion_type, ..
            }) => conversion_type.clone(),
            _ => ConversionType::Unknown,
        }
    }

    pub fn can_use_to_generate(&self) -> bool {
        matches!(self, ConversionType::Option | ConversionType::Result { .. })
    }
}
