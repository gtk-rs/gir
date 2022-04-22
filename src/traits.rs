pub use crate::config::{matchable::Matchable, parameter_matchable::ParameterMatchable};

pub trait AsStr {
    fn as_str(&self) -> &str;
}

pub trait IntoString {
    fn into_string(self) -> String;
}

pub trait MapAny<T> {
    fn map_any<F: FnOnce(T) -> T>(self, op: F) -> Self;
}

impl<T> MapAny<T> for Result<T, T> {
    fn map_any<F: FnOnce(T) -> T>(self, op: F) -> Self {
        match self {
            Ok(x) => Ok(op(x)),
            Err(x) => Err(op(x)),
        }
    }
}

pub trait MaybeRef<T> {
    fn maybe_ref(&self) -> Option<&T>;
    fn to_ref(&self) -> &T;
}

pub trait MaybeRefAs {
    fn maybe_ref_as<T>(&self) -> Option<&T>
    where
        Self: MaybeRef<T>;
    fn to_ref_as<T>(&self) -> &T
    where
        Self: MaybeRef<T>;
}
