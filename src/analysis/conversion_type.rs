use library::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversionType {
    Direct,     //coded without conversion
    Scalar,     //coded with from_glib
    Pointer,    //coded with from_glib_xxx
    Unknown,
}

impl Default for ConversionType {
    fn default() -> Self { ConversionType::Unknown }
}

impl ConversionType {
    pub fn of(library: &Library, type_id: TypeId) -> ConversionType {
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
                UniChar => ConversionType::Unknown,
                Pointer => ConversionType::Pointer,
                VarArgs => ConversionType::Unknown,
                Utf8 => ConversionType::Pointer,
                Filename => ConversionType::Pointer,
                Type => ConversionType::Scalar,
                None => ConversionType::Unknown,
                Unsupported => ConversionType::Unknown,
            },
            Alias(ref alias) => ConversionType::of(library, alias.typ),
            Bitfield(_) => ConversionType::Direct,
            Record(_) => ConversionType::Pointer,
            Enumeration(_) => ConversionType::Direct,
            Interface(_) => ConversionType::Pointer,
            Class(_) => ConversionType::Pointer,
            List(..) => ConversionType::Pointer,
            _ => ConversionType::Unknown,
        }
    }
}
