use library;
use super::c_type::is_mut_ptr;
use super::record_type::RecordType;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RefMode {
    None,
    ByRef,
    ByRefMut,
    ByRefImmut, //immutable reference with mutable pointer in ffi
    ByRefFake,
}

impl RefMode {
    #[inline]
    pub fn of(
        library: &library::Library,
        tid: library::TypeId,
        direction: library::ParameterDirection,
    ) -> RefMode {
        use library::Type::*;
        match *library.type_(tid) {
            Fundamental(library::Fundamental::Utf8) |
            Fundamental(library::Fundamental::Filename) |
            Class(..) |
            Interface(..) |
            List(..) |
            SList(..) |
            CArray(..) => {
                if direction == library::ParameterDirection::In {
                    RefMode::ByRef
                } else {
                    RefMode::None
                }
            }
            Record(ref record) => {
                if direction == library::ParameterDirection::In {
                    match RecordType::of(record) {
                        RecordType::Direct => RefMode::ByRefMut,
                        RecordType::Boxed => RefMode::ByRefMut,
                        RecordType::Refcounted => RefMode::ByRef,
                    }
                } else {
                    RefMode::None
                }
            }
            Union(..) => {
                if direction == library::ParameterDirection::In {
                    RefMode::ByRefMut
                } else {
                    RefMode::None
                }
            }
            Alias(ref alias) => RefMode::of(library, alias.typ, direction),
            _ => RefMode::None,
        }
    }

    pub fn without_unneeded_mut(
        library: &library::Library,
        par: &library::Parameter,
        immutable: bool,
    ) -> RefMode {
        let ref_mode = RefMode::of(library, par.typ, par.direction);
        if ref_mode == RefMode::ByRefMut {
            if !is_mut_ptr(&*par.c_type) {
                RefMode::ByRef
            } else if immutable {
                RefMode::ByRefImmut
            } else {
                ref_mode
            }
        } else {
            ref_mode
        }
    }

    pub fn is_ref(&self) -> bool {
        use self::RefMode::*;
        match *self {
            None => false,
            ByRef => true,
            ByRefMut => true,
            ByRefImmut => true,
            ByRefFake => true,
        }
    }
}
