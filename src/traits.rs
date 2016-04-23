pub use std::borrow::Cow;

pub use config::identables::Identables;

pub trait AsStr {
    fn as_str(&self) -> &str;
}

pub trait IntoString {
    fn into_string(self) -> String;
}

pub trait IntoStatic {
    type Static;
    fn into_static(self) -> Self::Static;
}

pub trait MapAny<'a, B: ?Sized + 'a>
where B: ToOwned {
    fn map_any<F: FnOnce(Cow<'a, B>) -> Cow<'a, B>>(self, op: F) -> Self;
}

pub trait MaybeRef<T> {
    fn maybe_ref(&self) -> Option<&T>;
    fn to_ref(&self) -> &T;
}

pub trait MaybeRefAs {
    fn maybe_ref_as<T>(&self) -> Option<&T> where Self: MaybeRef<T>;
    fn to_ref_as<T>(&self) -> &T where Self: MaybeRef<T>;
}
