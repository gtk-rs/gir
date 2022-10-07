use super::{c_type::is_mut_ptr, record_type::RecordType};
use crate::{config::gobjects::GObject, env, library};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RefMode {
    None,
    ByRef,
    ByRefMut,
    ByRefImmut, //immutable reference with mutable pointer in sys
    ByRefConst, //instance parameters in trait function with const pointer in sys
    ByRefFake,
}

impl RefMode {
    #[inline]
    pub fn of(
        env: &env::Env,
        tid: library::TypeId,
        direction: library::ParameterDirection,
    ) -> RefMode {
        use crate::library::Type::*;

        let library = &env.library;

        if let Some(&GObject {
            ref_mode: Some(ref_mode),
            ..
        }) = env.config.objects.get(&tid.full_name(library))
        {
            if direction == library::ParameterDirection::In {
                return ref_mode;
            } else {
                return RefMode::None;
            }
        }

        match library.type_(tid) {
            Basic(library::Basic::Utf8 | library::Basic::Filename | library::Basic::OsString)
            | Class(..)
            | Interface(..)
            | List(..)
            | SList(..)
            | PtrArray(..)
            | CArray(..) => {
                if direction == library::ParameterDirection::In {
                    RefMode::ByRef
                } else {
                    RefMode::None
                }
            }
            Record(record) => {
                if direction == library::ParameterDirection::In {
                    if let RecordType::Refcounted = RecordType::of(record) {
                        RefMode::ByRef
                    } else {
                        RefMode::ByRefMut
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
            Alias(alias) => RefMode::of(env, alias.typ, direction),
            _ => RefMode::None,
        }
    }

    pub fn without_unneeded_mut(
        env: &env::Env,
        par: &library::Parameter,
        immutable: bool,
        self_in_trait: bool,
    ) -> RefMode {
        use self::RefMode::*;
        let ref_mode = RefMode::of(env, par.typ, par.direction);
        match ref_mode {
            ByRefMut if !is_mut_ptr(&par.c_type) => ByRef,
            ByRefMut if immutable => ByRefImmut,
            ByRef if self_in_trait && !is_mut_ptr(&par.c_type) => ByRefConst,
            ref_mode => ref_mode,
        }
    }

    pub fn is_ref(self) -> bool {
        use self::RefMode::*;
        match self {
            None => false,
            ByRef => true,
            ByRefMut => true,
            ByRefImmut => true,
            ByRefConst => true,
            ByRefFake => true,
        }
    }
}
