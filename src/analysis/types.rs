use crate::library::*;

/// Array size limit above which Rust no longer automatically derives traits.
const RUST_DERIVE_ARRAY_SIZE_LIMIT: u16 = 32;
/// Number of parameters above which Rust no longer automatically derives traits in functions.
const RUST_DERIVE_PARAM_SIZE_LIMIT: usize = 12;

/// Checks if given type is some kind of pointer.
pub trait IsPtr {
    fn is_ptr(&self) -> bool;
}

impl IsPtr for Field {
    fn is_ptr(&self) -> bool {
        if let Some(ref c_type) = self.c_type {
            c_type.contains('*')
        } else {
            // After library post processing phase
            // only types without c:type should be
            // function pointers, we need check their parameters.
            false
        }
    }
}

impl IsPtr for Alias {
    fn is_ptr(&self) -> bool {
        self.target_c_type.contains('*')
    }
}

/// Checks if given type has volatile qualifier.
pub trait IsVolatile {
    fn is_volatile(&self) -> bool;
}

impl IsVolatile for Field {
    fn is_volatile(&self) -> bool {
        if let Some(ref c_type) = self.c_type {
            c_type.starts_with("volatile")
        } else {
            false
        }
    }
}

/// Checks if given type is incomplete, i.e., its size is unknown.
pub trait IsIncomplete {
    fn is_incomplete(&self, lib: &Library) -> bool;
}

impl IsIncomplete for Fundamental {
    fn is_incomplete(&self, _lib: &Library) -> bool {
        matches!(
            *self,
            Fundamental::None | Fundamental::Unsupported | Fundamental::VarArgs
        )
    }
}

impl IsIncomplete for Alias {
    fn is_incomplete(&self, lib: &Library) -> bool {
        if self.is_ptr() {
            false
        } else {
            lib.type_(self.typ).is_incomplete(lib)
        }
    }
}

impl IsIncomplete for Field {
    fn is_incomplete(&self, lib: &Library) -> bool {
        if self.is_ptr() {
            // Pointers are always complete.
            false
        } else {
            lib.type_(self.typ).is_incomplete(lib)
        }
    }
}

impl<'a> IsIncomplete for &'a [Field] {
    fn is_incomplete(&self, lib: &Library) -> bool {
        if self.is_empty() {
            return true;
        }

        let mut is_bitfield = false;
        for field in self.iter() {
            if field.is_incomplete(lib) {
                return true;
            }
            // Two consequitive bitfields are unrepresentable in Rust,
            // so from our perspective they are incomplete.
            if is_bitfield && field.bits.is_some() {
                return true;
            }
            is_bitfield = field.bits.is_some();
        }

        false
    }
}

impl IsIncomplete for Class {
    fn is_incomplete(&self, lib: &Library) -> bool {
        self.fields.as_slice().is_incomplete(lib)
    }
}

impl IsIncomplete for Record {
    #[allow(clippy::if_same_then_else)]
    fn is_incomplete(&self, lib: &Library) -> bool {
        if self.c_type == "GHookList" {
            // Search for GHookList in sys codegen for rationale.
            false
        } else if self.disguised {
            false
        } else {
            self.fields.as_slice().is_incomplete(lib)
        }
    }
}

impl IsIncomplete for Union {
    fn is_incomplete(&self, lib: &Library) -> bool {
        self.fields.as_slice().is_incomplete(lib)
    }
}

impl IsIncomplete for Function {
    fn is_incomplete(&self, lib: &Library) -> bool {
        //Checking p.typ.is_incomplete(lib) cause recursive check on GScannerMsgFunc
        self.parameters.iter().any(|p| {
            matches!(
                lib.type_(p.typ),
                Type::Fundamental(Fundamental::Unsupported)
                    | Type::Fundamental(Fundamental::VarArgs)
            )
        })
    }
}

impl IsIncomplete for TypeId {
    fn is_incomplete(&self, lib: &Library) -> bool {
        lib.type_(*self).is_incomplete(lib)
    }
}

impl IsIncomplete for Type {
    fn is_incomplete(&self, lib: &Library) -> bool {
        match *self {
            Type::Fundamental(ref fundamental) => fundamental.is_incomplete(lib),
            Type::Alias(ref alias) => alias.is_incomplete(lib),
            Type::FixedArray(tid, ..) => tid.is_incomplete(lib),
            Type::Class(ref klass) => klass.is_incomplete(lib),
            Type::Record(ref record) => record.is_incomplete(lib),
            Type::Union(ref union) => union.is_incomplete(lib),
            Type::Function(ref function) => function.is_incomplete(lib),
            Type::Interface(..) => true,
            Type::Custom(..)
            | Type::Enumeration(..)
            | Type::Bitfield(..)
            | Type::Array(..)
            | Type::CArray(..)
            | Type::PtrArray(..)
            | Type::HashTable(..)
            | Type::List(..)
            | Type::SList(..) => false,
        }
    }
}

/// Checks if type is external aka opaque type.
pub trait IsExternal {
    fn is_external(&self, lib: &Library) -> bool;
}

impl IsExternal for Class {
    fn is_external(&self, _lib: &Library) -> bool {
        self.fields.is_empty()
    }
}

impl IsExternal for Record {
    fn is_external(&self, _lib: &Library) -> bool {
        self.fields.is_empty()
    }
}

impl IsExternal for Union {
    fn is_external(&self, _lib: &Library) -> bool {
        self.fields.is_empty()
    }
}

impl IsExternal for Alias {
    fn is_external(&self, lib: &Library) -> bool {
        if self.is_ptr() {
            false
        } else {
            lib.type_(self.typ).is_external(lib)
        }
    }
}

impl IsExternal for Type {
    fn is_external(&self, lib: &Library) -> bool {
        match *self {
            Type::Alias(ref alias) => alias.is_external(lib),
            Type::Class(ref klass) => klass.is_external(lib),
            Type::Record(ref record) => record.is_external(lib),
            Type::Union(ref union) => union.is_external(lib),
            Type::Interface(..) => true,
            Type::Custom(..)
            | Type::Fundamental(..)
            | Type::Enumeration(..)
            | Type::Bitfield(..)
            | Type::Function(..)
            | Type::Array(..)
            | Type::CArray(..)
            | Type::FixedArray(..)
            | Type::PtrArray(..)
            | Type::HashTable(..)
            | Type::List(..)
            | Type::SList(..) => false,
        }
    }
}

/// Checks if given type derives Copy trait.
pub trait DerivesCopy {
    fn derives_copy(&self, lib: &Library) -> bool;
}

impl DerivesCopy for Fundamental {
    fn derives_copy(&self, lib: &Library) -> bool {
        !self.is_incomplete(lib)
            && !matches!(
                self,
                Fundamental::Pointer
                    | Fundamental::VarArgs
                    | Fundamental::Utf8
                    | Fundamental::Filename
                    | Fundamental::IntPtr
                    | Fundamental::UIntPtr
                    | Fundamental::OsString
                    | Fundamental::Unsupported
            )
    }
}

impl DerivesCopy for Alias {
    fn derives_copy(&self, lib: &Library) -> bool {
        !self.is_incomplete(lib) && !self.is_ptr()
    }
}

impl DerivesCopy for Field {
    fn derives_copy(&self, lib: &Library) -> bool {
        !self.is_incomplete(lib) && !self.is_ptr()
    }
}

impl<'a> DerivesCopy for &'a [Field] {
    fn derives_copy(&self, lib: &Library) -> bool {
        !self.is_incomplete(lib) && !self.iter().any(|field| field.is_ptr())
    }
}

impl DerivesCopy for Class {
    fn derives_copy(&self, lib: &Library) -> bool {
        !self.is_incomplete(lib) && self.fields.as_slice().derives_copy(lib)
    }
}

impl DerivesCopy for Record {
    fn derives_copy(&self, lib: &Library) -> bool {
        !self.is_incomplete(lib) && self.fields.as_slice().derives_copy(lib)
    }
}

impl DerivesCopy for Union {
    fn derives_copy(&self, lib: &Library) -> bool {
        !self.is_incomplete(lib) && self.fields.as_slice().derives_copy(lib)
    }
}

impl DerivesCopy for Function {
    fn derives_copy(&self, lib: &Library) -> bool {
        !self.is_incomplete(lib)
    }
}

impl DerivesCopy for TypeId {
    fn derives_copy(&self, lib: &Library) -> bool {
        !self.is_incomplete(lib) && lib.type_(*self).derives_copy(lib)
    }
}

impl DerivesCopy for Type {
    fn derives_copy(&self, lib: &Library) -> bool {
        if self.is_incomplete(lib) {
            return false;
        }

        match *self {
            Type::Fundamental(ref fundamental) => fundamental.derives_copy(lib),
            Type::Alias(ref alias) => alias.derives_copy(lib),
            Type::FixedArray(tid, ..) => tid.derives_copy(lib),
            Type::Class(ref klass) => klass.derives_copy(lib),
            Type::Record(ref record) => record.derives_copy(lib),
            Type::Union(ref union) => union.derives_copy(lib),
            Type::Function(ref function) => function.derives_copy(lib),
            Type::Enumeration(..) | Type::Bitfield(..) | Type::Interface(..) => true,
            Type::Custom(..)
            | Type::Array(..)
            | Type::CArray(..)
            | Type::PtrArray(..)
            | Type::HashTable(..)
            | Type::List(..)
            | Type::SList(..) => false,
        }
    }
}

/// Checks if given type implements Debug trait.
pub trait ImplementsDebug {
    fn implements_debug(&self, lib: &Library) -> bool;
}

impl ImplementsDebug for Field {
    fn implements_debug(&self, lib: &Library) -> bool {
        match *lib.type_(self.typ) {
            Type::FixedArray(_, size, _) => size <= RUST_DERIVE_ARRAY_SIZE_LIMIT,
            Type::Function(Function { ref parameters, .. }) => {
                parameters.len() <= RUST_DERIVE_PARAM_SIZE_LIMIT
            }
            _ => true,
        }
    }
}
