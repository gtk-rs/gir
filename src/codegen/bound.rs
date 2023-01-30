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
    pub(super) fn type_parameter_reference(&self) -> Option<char> {
        self.alias
    }

    /// Returns the type parameter reference, with [`BoundType::IsA`] wrapped
    /// in `ref_mode` and `nullable` as appropriate.
    pub(super) fn full_type_parameter_reference(
        &self,
        ref_mode: RefMode,
        nullable: Nullable,
        r#async: bool,
    ) -> String {
        let ref_str = ref_mode.for_rust_type();

        // Generate `impl Trait` if this bound does not have an alias
        let trait_bound = match self.type_parameter_reference() {
            Some(t) => t.to_string(),
            None => {
                let trait_bound = self.trait_bound(r#async);
                let trait_bound = format!("impl {trait_bound}");

                // Combining a ref mode and lifetime requires parentheses for disambiguation
                match self.bound_type {
                    BoundType::IsA(lifetime) => {
                        // TODO: This is fragile
                        let has_lifetime = r#async || lifetime.is_some();

                        if !ref_str.is_empty() && has_lifetime {
                            format!("({trait_bound})")
                        } else {
                            trait_bound
                        }
                    }
                    _ => trait_bound,
                }
            }
        };

        match self.bound_type {
            BoundType::IsA(_) if *nullable => {
                format!("Option<{ref_str}{trait_bound}>")
            }
            BoundType::IsA(_) => format!("{ref_str}{trait_bound}"),
            BoundType::AsRef(_) if *nullable => {
                format!("Option<{trait_bound}>")
            }
            BoundType::NoWrapper | BoundType::AsRef(_) => trait_bound,
        }
    }

    /// Returns the type parameter definition for this bound, usually
    /// of the form `T: SomeTrait` or `T: IsA<Foo>`.
    pub(super) fn type_parameter_definition(&self, r#async: bool) -> Option<String> {
        self.alias
            .map(|alias| format!("{}: {}", alias, self.trait_bound(r#async)))
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
                    .or_else(|| lifetime.map(|l| format!(" + '{l}")))
                    .unwrap_or_default();

                format!("{is_a}{lifetime}")
            }
            BoundType::AsRef(Some(_ /* lifetime */)) => panic!("AsRef cannot have a lifetime"),
            BoundType::AsRef(None) => format!("AsRef<{}>", self.type_str),
        }
    }
}
