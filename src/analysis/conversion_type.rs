use env;
use library::*;
use config::gobjects::GObject;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversionType {
    Direct,  //coded without conversion
    Scalar,  //coded with from_glib
    Pointer, //coded with from_glib_xxx
    Borrow,  //same as Pointer, except that use from_glib_borrow instead from_glib_none
    Unknown,
}

impl Default for ConversionType {
    fn default() -> Self {
        ConversionType::Unknown
    }
}

impl ConversionType {
    pub fn of(env: &env::Env, type_id: TypeId) -> ConversionType {
        let library = &env.library;

        if let Some(&GObject {
            conversion_type: Some(conversion_type),
            ..
        }) = env.config.objects.get(&type_id.full_name(library))
        {
            return conversion_type;
        }

        use library::Type::*;
        use library::Fundamental::*;
        match *library.type_(type_id) {
            Fundamental(fund) => match fund {
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
                Unsupported => ConversionType::Unknown,
            },
            Alias(ref alias) if alias.c_identifier == "GQuark" => ConversionType::Scalar,
            Alias(ref alias) => ConversionType::of(env, alias.typ),
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
            Function(super::library::Function { ref name, .. }) if name == "AsyncReadyCallback" =>
                ConversionType::Direct,
            Custom(super::library::Custom {
                conversion_type, ..
            }) => conversion_type,
            _ => ConversionType::Unknown,
        }
    }
}
