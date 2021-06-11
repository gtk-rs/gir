use crate::{
    analysis::{
        bounds::{Bound, BoundType},
        ref_mode::RefMode,
    },
    library::Nullable,
};

impl Bound {
    /// Returns the type parameter reference.
    /// Currently always returns the alias.
    pub(super) fn type_parameter_reference(&self) -> char {
        self.alias
    }

    /// Returns the type parameter reference, with [`BoundType::IsA`] wrapped
    /// in `ref_mode` and `nullable` as appropriate.
    pub(super) fn full_type_parameter_reference(
        &self,
        ref_mode: RefMode,
        nullable: Nullable,
    ) -> String {
        let t = self.type_parameter_reference();
        let ref_str = ref_mode.for_rust_type();
        match self.bound_type {
            BoundType::IsA(_) if *nullable => {
                format!("Option<{}{}>", ref_str, t)
            }
            BoundType::IsA(_) => format!("{}{}", ref_str, t),
            BoundType::Into(_) if *nullable => {
                format!("Option<{}>", t)
            }
            BoundType::Into(_) => {
                format!("{}", t)
            }
            BoundType::NoWrapper | BoundType::AsRef(_) => t.to_string(),
        }
    }

    /// Returns the type parameter definition for this bound, usually
    /// of the form `T: SomeTrait` or `T: IsA<Foo>`.
    pub(super) fn type_parameter_definition(&self, r#async: bool) -> String {
        format!("{}: {}", self.alias, self.trait_bound(r#async))
    }

    /// Returns the trait bound, usually of the form `SomeTrait`
    /// or `IsA<Foo>`.
    pub(super) fn trait_bound(&self, r#async: bool) -> String {
        match self.bound_type {
            BoundType::NoWrapper => self.type_str.clone(),
            BoundType::IsA(lifetime) => {
                if r#async {
                    assert!(lifetime.is_none(), "Async overwrites lifetime");
                }
                let is_a = format!("IsA<{}>", self.type_str);
                let lifetime = r#async
                    .then(|| " + Clone + 'static".to_string())
                    .or_else(|| lifetime.map(|l| format!(" + '{}", l)))
                    .unwrap_or_default();

                format!("{}{}", is_a, lifetime)
            }
            BoundType::AsRef(Some(_ /*lifetime*/)) => panic!("AsRef cannot have a lifetime"),
            BoundType::AsRef(None) => format!("AsRef<{}>", self.type_str),
            BoundType::Into(Some(_ /*lifetime*/)) => panic!("Into cannot have a lifetime"),
            BoundType::Into(None) => {
                let is_a = format!("Into<{}>", self.type_str);
                let lifetime = r#async
                    .then(|| " + Clone + 'static".to_string())
                    .or_else(|| Some("".to_string()))
                    .unwrap_or_default();

                format!("{}{}", is_a, lifetime)
            }
        }
    }
}
