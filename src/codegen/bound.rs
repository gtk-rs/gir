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
    ///
    /// This doesn't include the lifetime, which could be shared by other type
    /// parameters. Use [Bounds::to_generic_params_str](crate::analysis::bounds::Bounds::to_generic_params_str)
    /// to get the full generic parameter list, including lifetimes.
    pub fn type_parameter_definition(&self, r#async: bool) -> Option<String> {
        use BoundType::*;
        match self.bound_type {
            NoWrapper => {
                let alias = self.alias.expect("must be defined in this context");
                Some(format!("{alias}: {}", self.type_str))
            }
            IsA if self.alias.is_some() => Some(format!(
                "{}: IsA<{}>{}",
                self.alias.unwrap(),
                self.type_str,
                if r#async { " + Clone + 'static" } else { "" },
            )),
            IntoOptionIsA => {
                let alias = self.alias.expect("must be defined in this context");
                Some(format!(
                    "{alias}: IsA<{}>{}",
                    self.type_str,
                    if r#async { " + Clone + 'static" } else { "" },
                ))
            }
            _ => None,
        }
    }

    /// Returns the type parameter reference, with [`BoundType::IsA`] wrapped
    /// in `ref_mode` and `nullable` as appropriate.
    pub fn full_type_parameter_reference(
        &self,
        ref_mode: RefMode,
        nullable: Nullable,
        r#async: bool,
    ) -> String {
        use BoundType::*;
        match self.bound_type {
            NoWrapper => self
                .alias
                .expect("must be defined in this context")
                .to_string(),
            IsA if self.alias.is_none() => {
                let suffix = r#async
                    .then(|| " + Clone + 'static".to_string())
                    .unwrap_or_default();

                let mut trait_bound = format!("impl IsA<{}>{suffix}", self.type_str);

                let ref_str = ref_mode.to_string();
                if !ref_str.is_empty() && r#async {
                    trait_bound = format!("({trait_bound})");
                }

                if *nullable {
                    format!("Option<{ref_str}{trait_bound}>")
                } else {
                    format!("{ref_str}{trait_bound}")
                }
            }
            IsA if self.alias.is_some() => {
                let alias = self.alias.unwrap();
                let ref_str = ref_mode.to_string();
                if *nullable {
                    format!("Option<{ref_str} {alias}>")
                } else {
                    format!("{ref_str} {alias}")
                }
            }
            IsA => {
                if *nullable {
                    format!("Option<impl Isa<{}>>", self.type_str)
                } else {
                    format!("impl IsA<{}>", self.type_str)
                }
            }
            AsRef => {
                assert!(self.lt.is_none(), "AsRef cannot have a lifetime");

                if *nullable {
                    format!("Option<impl AsRef<{}>>", self.type_str)
                } else {
                    format!("impl AsRef<{}>", self.type_str)
                }
            }
            IntoOption => {
                format!("impl Into<Option<{}>>", self.type_str)
            }
            IntoOptionRef => {
                assert!(self.lt.is_some(), "must be defined in this context");
                let ref_str = ref_mode.to_string_with_maybe_lt(self.lt);

                format!("impl Into<Option<{ref_str} {}>>", self.type_str)
            }
            IntoOptionIsA => {
                assert!(self.lt.is_some(), "must be defined in this context");
                let ref_str = ref_mode.to_string_with_maybe_lt(self.lt);
                let alias = self.alias.expect("must be defined in this context");

                format!("impl Into<Option<{ref_str} {alias}>>")
            }
        }
    }
}
