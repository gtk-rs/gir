use library;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RefMode {
    None,
    ByRef,
}

impl RefMode {
    #[inline]
    pub fn of(library: &library::Library, tid: library::TypeId, direction: library::ParameterDirection) -> RefMode {
        use library::Type::*;
        match *library.type_(tid) {
            Fundamental(library::Fundamental::Utf8) |
            Fundamental(library::Fundamental::Filename) |
            Record(..) |
            Class(..) |
            Interface(..) |
            List(..) => if direction == library::ParameterDirection::In {
                RefMode::ByRef
            } else {
                RefMode::None
            },
            Alias(ref alias) => RefMode::of(library, alias.typ, direction),
            _ => RefMode::None,
        }
    }

    pub fn is_ref(&self) -> bool {
        use self::RefMode::*;
        match *self {
            None => false,
            ByRef => true,
        }
    }
}
