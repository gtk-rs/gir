use std::{borrow::Cow, sync::Arc};

use crate::{
    analysis::conversion_type::ConversionType,
    config,
    library::{self, Infallible, Mandatory},
    Env,
};

#[derive(Default, Clone, Debug)]
pub enum TryFromGlib {
    #[default]
    Default,
    NotImplemented,
    Option,
    OptionMandatory,
    Result {
        ok_type: Arc<str>,
        err_type: Arc<str>,
    },
    ResultInfallible {
        ok_type: Arc<str>,
    },
}

impl TryFromGlib {
    fn _new(
        env: &Env,
        type_id: library::TypeId,
        mut config_mandatory: impl Iterator<Item = Mandatory>,
        mut config_infallible: impl Iterator<Item = Infallible>,
    ) -> Self {
        let conversion_type = ConversionType::of(env, type_id);
        match conversion_type {
            ConversionType::Option => {
                if *config_mandatory.next().unwrap_or(Mandatory(false)) {
                    TryFromGlib::OptionMandatory
                } else {
                    TryFromGlib::Option
                }
            }
            ConversionType::Result { ok_type, err_type } => {
                if *config_infallible.next().unwrap_or(Infallible(false)) {
                    TryFromGlib::ResultInfallible {
                        ok_type: Arc::clone(&ok_type),
                    }
                } else {
                    TryFromGlib::Result {
                        ok_type: Arc::clone(&ok_type),
                        err_type: Arc::clone(&err_type),
                    }
                }
            }
            _ => TryFromGlib::NotImplemented,
        }
    }

    pub fn from_type_defaults(env: &Env, type_id: library::TypeId) -> Self {
        Self::_new(env, type_id, None.into_iter(), None.into_iter())
    }

    pub fn or_type_defaults(&self, env: &Env, type_id: library::TypeId) -> Cow<'_, Self> {
        match self {
            TryFromGlib::Default => Cow::Owned(Self::from_type_defaults(env, type_id)),
            other => Cow::Borrowed(other),
        }
    }

    pub fn from_parameter(
        env: &Env,
        type_id: library::TypeId,
        configured_parameters: &[&config::functions::Parameter],
    ) -> Self {
        Self::_new(
            env,
            type_id,
            configured_parameters.iter().filter_map(|par| par.mandatory),
            configured_parameters
                .iter()
                .filter_map(|par| par.infallible),
        )
    }

    pub fn from_return_value(
        env: &Env,
        type_id: library::TypeId,
        configured_functions: &[&config::functions::Function],
    ) -> Self {
        Self::_new(
            env,
            type_id,
            configured_functions.iter().filter_map(|f| f.ret.mandatory),
            configured_functions.iter().filter_map(|f| f.ret.infallible),
        )
    }
}
