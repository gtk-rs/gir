use std::{
    fmt::{self, Write},
    str::FromStr,
};

use super::{c_type::is_mut_ptr, record_type::RecordType};
use crate::{config::gobjects::GObject, env, library};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RefMode {
    None,
    ByRef,
    ByRefMut,
    ByRefImmut, // immutable reference with mutable pointer in sys
    ByRefConst, // instance parameters in trait function with const pointer in sys
    ByRefFake,
}

impl FromStr for RefMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "ref" => Ok(Self::ByRef),
            "ref-mut" => Ok(Self::ByRefMut),
            "ref-immut" => Ok(Self::ByRefImmut),
            "ref-fake" => Ok(Self::ByRefFake),
            name => Err(format!("Unknown reference mode '{name}'")),
        }
    }
}

impl RefMode {
    #[inline]
    pub fn of(
        env: &env::Env,
        tid: library::TypeId,
        direction: library::ParameterDirection,
    ) -> Self {
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
                return Self::None;
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
                    Self::ByRef
                } else {
                    Self::None
                }
            }
            Record(record) => {
                if direction == library::ParameterDirection::In {
                    if let RecordType::Refcounted = RecordType::of(record) {
                        Self::ByRef
                    } else {
                        Self::ByRefMut
                    }
                } else {
                    Self::None
                }
            }
            Union(..) => {
                if direction == library::ParameterDirection::In {
                    Self::ByRefMut
                } else {
                    Self::None
                }
            }
            Alias(alias) => Self::of(env, alias.typ, direction),
            _ => Self::None,
        }
    }

    pub fn without_unneeded_mut(
        env: &env::Env,
        par: &library::Parameter,
        immutable: bool,
        self_in_trait: bool,
    ) -> Self {
        let ref_mode = Self::of(env, par.typ, par.direction);
        match ref_mode {
            Self::ByRefMut if !is_mut_ptr(&par.c_type) => Self::ByRef,
            Self::ByRefMut if immutable => Self::ByRefImmut,
            Self::ByRef if self_in_trait && !is_mut_ptr(&par.c_type) => Self::ByRefConst,
            Self::None if par.direction.is_out() && !*par.nullable => Self::ByRefMut,
            ref_mode => ref_mode,
        }
    }

    pub fn is_ref(self) -> bool {
        match self {
            Self::None => false,
            Self::ByRef
            | Self::ByRefMut
            | Self::ByRefImmut
            | Self::ByRefConst
            | Self::ByRefFake => true,
        }
    }

    pub fn is_immutable(self) -> bool {
        match self {
            Self::None | Self::ByRefMut => false,
            Self::ByRef | Self::ByRefImmut | Self::ByRefConst | Self::ByRefFake => true,
        }
    }

    pub fn is_none(self) -> bool {
        matches!(self, Self::None)
    }

    pub fn to_string_with_maybe_lt(self, lt: Option<char>) -> String {
        match self {
            RefMode::None | RefMode::ByRefFake => {
                assert!(lt.is_none(), "incompatible ref mode {self:?} with lifetime");
                String::new()
            }
            RefMode::ByRef | RefMode::ByRefImmut | RefMode::ByRefConst => {
                if let Some(lt) = lt {
                    format!("&'{lt}")
                } else {
                    "&".to_string()
                }
            }
            RefMode::ByRefMut => {
                if let Some(lt) = lt {
                    format!("&'{lt} mut ")
                } else {
                    "&mut ".to_string()
                }
            }
        }
    }
}

impl fmt::Display for RefMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefMode::None | RefMode::ByRefFake => f.write_str(""),
            RefMode::ByRef | RefMode::ByRefImmut | RefMode::ByRefConst => f.write_char('&'),
            RefMode::ByRefMut => f.write_str("&mut "),
        }
    }
}
