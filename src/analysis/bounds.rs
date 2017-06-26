use std::collections::vec_deque::VecDeque;
use std::slice::Iter;
use std::vec::Vec;

use consts::TYPE_PARAMETERS_START;
use env::Env;
use analysis::imports::Imports;
use analysis::parameter::Parameter;
use analysis::rust_type::bounds_rust_type;
use library::{Function, Fundamental, Nullable, Type, TypeId, ParameterDirection};
use traits::IntoString;

#[derive(Clone, Eq, Debug, PartialEq)]
pub enum BoundType {
    // lifetime
    IsA(Option<char>),
    // lifetime <- shouldn't be used but just in case...
    AsRef(Option<char>),
    // lifetime (if none, not a reference, like for primitive types), wrapper type, is mutable
    Into(Option<char>, Option<Box<BoundType>>, bool),
}

impl BoundType {
    pub fn is_into(&self) -> bool {
        match *self {
            BoundType::Into(_, _, _) => true,
            _ => false,
        }
    }

    fn with_lifetime(ty_: BoundType, lifetime: char) -> BoundType {
        match ty_ {
            BoundType::IsA(_) => BoundType::IsA(Some(lifetime)),
            BoundType::AsRef(_) => BoundType::AsRef(Some(lifetime)),
            BoundType::Into(_, x, mutable) => BoundType::Into(Some(lifetime), x, mutable),
        }
    }
}

#[derive(Clone, Eq, Debug, PartialEq)]
pub struct Bound {
    pub bound_type: BoundType,
    pub parameter_name: String,
    pub alias: char,
    pub type_str: String,
    pub info_for_next_type: bool,
}

#[derive(Clone, Debug)]
pub struct Bounds {
    unused: VecDeque<char>,
    used: Vec<Bound>,
    unused_lifetimes: VecDeque<char>,
    lifetimes: Vec<char>,
}

impl Default for Bounds {
    #[cfg_attr(feature = "cargo-clippy", allow(char_lit_as_u8))]
    fn default() -> Bounds {
        Bounds {
            unused: (TYPE_PARAMETERS_START as u8..)
                .take_while(|x| *x <= 'Z' as u8)
                .map(|x| x as char)
                .collect(),
            used: Vec::new(),
            unused_lifetimes: "abcdefg".chars().collect(),
            lifetimes: Vec::new(),
        }
    }
}

impl Bound {
    pub fn get_for_property_setter(
        env: &Env,
        var_name: &str,
        type_id: TypeId,
        nullable: Nullable,
    ) -> Option<Bound> {
        match Bounds::type_for(env, type_id, nullable, false) {
            //TODO: match boxed_bound to BoundType::IsA(l)
            Some(BoundType::Into(_, Some(boxed_bound), _)) => {
                let type_str = bounds_rust_type(env, type_id);
                Some(Bound {
                    bound_type: *boxed_bound.clone(),
                    parameter_name: var_name.to_owned(),
                    alias: TYPE_PARAMETERS_START,
                    type_str: type_str.into_string(),
                    info_for_next_type: false,
                })
            }
            _ => None,
        }
    }
}

impl Bounds {
    pub fn add_for_parameter(&mut self, env: &Env, func: &Function, par: &mut Parameter) {
        if !par.instance_parameter && par.direction != ParameterDirection::Out {
            if let Some(bound_type) = Bounds::type_for(env, par.typ, par.nullable,
                                                       par.ref_mode.is_ref_mut()) {
                par.to_glib_extra = Bounds::get_to_glib_extra(bound_type.clone());
                let type_name = bounds_rust_type(env, par.typ);
                if !self.add_parameter(&par.name, &type_name.into_string(), bound_type) {
                    panic!(
                        "Too many type constraints for {}",
                        func.c_identifier.as_ref().unwrap()
                    )
                }
            }
        }
    }

    pub fn type_for(env: &Env, type_id: TypeId, nullable: Nullable,
                    mutable: bool) -> Option<BoundType> {
        use self::BoundType::*;
        match *env.library.type_(type_id) {
            Type::Fundamental(Fundamental::Filename) => Some(AsRef(None)),
            Type::Fundamental(Fundamental::Utf8) if *nullable => Some(Into(Some('_'), None, mutable)),
            Type::Class(..) if !*nullable => {
                if env.class_hierarchy.subtypes(type_id).next().is_some() {
                    Some(IsA(None))
                } else {
                    None
                }
            }
            Type::Class(..) => {
                if env.class_hierarchy.subtypes(type_id).next().is_some() {
                    Some(Into(Some('_'), Some(Box::new(IsA(None))), mutable))
                } else {
                    Some(Into(Some('_'), None, mutable))
                }
            }
            Type::Interface(..) if !*nullable => Some(IsA(None)),
            Type::Interface(..) => Some(Into(Some('_'), Some(Box::new(IsA(None))), mutable)),
            Type::List(_) | Type::SList(_) | Type::CArray(_) => None,
            Type::Fundamental(_) if *nullable => Some(Into(None, None, mutable)),
            _ if !*nullable => None,
            _ => Some(Into(Some('_'), None, mutable)),
        }
    }
    fn get_to_glib_extra(bound_type: BoundType) -> String {
        use self::BoundType::*;
        match bound_type {
            AsRef(_) => ".as_ref()".to_owned(),
            Into(_, Some(x), _) => Bounds::get_to_glib_extra(*x),
            _ => String::new(),
        }
    }
    pub fn add_parameter(&mut self, name: &str, type_str: &str, mut bound_type: BoundType) -> bool {
        if self.used.iter().any(|n| n.parameter_name == name) {
            return false;
        }
        let sub = if let BoundType::Into(Some(_), x, mutable) = bound_type {
            if let Some(lifetime) = self.unused_lifetimes.pop_front() {
                self.lifetimes.push(lifetime);
                bound_type = BoundType::Into(Some(lifetime), x.clone(), mutable);
                Some((x, lifetime))
            } else {
                return false;
            }
        } else {
            None
        };
        let type_str = if let Some((Some(sub), lifetime)) = sub {
            if let Some(alias) = self.unused.pop_front() {
                self.used.push(Bound {
                    bound_type: BoundType::with_lifetime(*sub, lifetime),
                    parameter_name: name.to_owned(),
                    alias: alias,
                    type_str: type_str.to_owned(),
                    info_for_next_type: true,
                });
                alias.to_string()
            } else {
                return false;
            }
        } else {
            type_str.to_owned()
        };
        if let Some(alias) = self.unused.pop_front() {
            self.used.push(Bound {
                bound_type: bound_type,
                parameter_name: name.to_owned(),
                alias: alias,
                type_str: type_str.to_owned(),
                info_for_next_type: false,
            });
            true
        } else {
            false
        }
    }
    pub fn get_parameter_alias_info(&self, name: &str) -> Option<(char, BoundType)> {
        self.used
            .iter()
            .find(move |n| if n.parameter_name == name {
                !n.info_for_next_type
            } else {
                false
            })
            .map(|t| (t.alias, t.bound_type.clone()))
    }
    pub fn update_imports(&self, imports: &mut Imports) {
        //TODO: import with versions
        use self::BoundType::*;
        for used in &self.used {
            match used.bound_type {
                IsA(_) => imports.add("glib::object::IsA", None),
                AsRef(_) => imports.add_used_type(&used.type_str, None),
                Into(_, Some(ref x), _) => {
                    match **x {
                        IsA(_) => imports.add("glib::object::IsA", None),
                        // This case shouldn't be possible normally.
                        AsRef(_) => imports.add_used_type(&used.type_str, None),
                        _ => {}
                    }
                }
                Into(_, None, _) => {}
            }
        }
    }
    pub fn is_empty(&self) -> bool {
        self.used.is_empty()
    }
    pub fn iter(&self) -> Iter<Bound> {
        self.used.iter()
    }
    pub fn iter_lifetimes(&self) -> Iter<char> {
        self.lifetimes.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_new_all() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::IsA(None);
        assert_eq!(bounds.add_parameter("a", "", typ.clone()), true);
        // Don't add second time
        assert_eq!(bounds.add_parameter("a", "", typ.clone()), false);
        assert_eq!(bounds.add_parameter("b", "", typ.clone()), true);
        assert_eq!(bounds.add_parameter("c", "", typ.clone()), true);
        assert_eq!(bounds.add_parameter("d", "", typ.clone()), true);
        assert_eq!(bounds.add_parameter("e", "", typ.clone()), true);
        assert_eq!(bounds.add_parameter("f", "", typ.clone()), true);
        assert_eq!(bounds.add_parameter("g", "", typ.clone()), true);
        assert_eq!(bounds.add_parameter("h", "", typ.clone()), true);
        assert_eq!(bounds.add_parameter("h", "", typ.clone()), false);
        assert_eq!(bounds.add_parameter("i", "", typ.clone()), true);
        assert_eq!(bounds.add_parameter("j", "", typ.clone()), true);
        assert_eq!(bounds.add_parameter("k", "", typ.clone()), true);
        assert_eq!(bounds.add_parameter("l", "", typ), false);
    }

    #[test]
    fn get_parameter_alias_info() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::IsA(None);
        bounds.add_parameter("a", "", typ.clone());
        bounds.add_parameter("b", "", typ.clone());
        assert_eq!(
            bounds.get_parameter_alias_info("a"),
            Some(('P', typ.clone()))
        );
        assert_eq!(bounds.get_parameter_alias_info("b"), Some(('Q', typ)));
        assert_eq!(bounds.get_parameter_alias_info("c"), None);
    }
}
