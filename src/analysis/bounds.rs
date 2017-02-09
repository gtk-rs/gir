use std::collections::vec_deque::VecDeque;
use std::slice::Iter;
use std::vec::Vec;

use env::Env;
use library::{Fundamental, Library, Type, TypeId};
use super::imports::Imports;

#[derive(Copy, Clone, Eq, Debug, PartialEq)]
pub enum BoundType {
    IsA,
    AsRef,
    Into,
}

#[derive(Debug)]
pub struct Bounds {
    unused: VecDeque<String>,
    //Vector tuples <parameter name>, <alias>, <type>, <bound type>
    used: Vec<(String, String, String, BoundType)>,
    // In practice, it could be just a String since we only handle one lifetime.
    lifetimes: Vec<String>,
}

impl Default for Bounds {
    fn default () -> Bounds {
        Bounds {
            unused: "TUVWXYZ".chars().map(|ch| ch.to_string()).collect(),
            used: Vec::new(),
            lifetimes: Vec::new(),
        }
    }
}

impl Bounds {
    pub fn type_for(env: &Env, type_id: TypeId) -> Option<BoundType> {
        use self::BoundType::*;
        match *env.library.type_(type_id) {
            Type::Fundamental(Fundamental::Filename) => Some(AsRef),
            Type::Fundamental(Fundamental::Utf8) => Some(Into),
            Type::Class(..) => {
                if env.class_hierarchy.subtypes(type_id).next().is_some() {
                    Some(IsA)
                } else {
                    None
                }
            }
            Type::Interface(..) => Some(IsA),
            _ => None,
        }
    }
    pub fn to_glib_extra(library: &Library, type_id: TypeId, is_nullable: bool) -> String {
        match *library.type_(type_id) {
            Type::Fundamental(Fundamental::Filename) => ".as_ref()".to_owned(),
            Type::Fundamental(Fundamental::Utf8) if is_nullable => ".into()".to_owned(),
            _ => String::new(),
        }
    }
    pub fn get_cast(library: &Library, type_id: TypeId, is_nullable: bool) -> String {
        match *library.type_(type_id) {
            Type::Fundamental(Fundamental::Utf8) if is_nullable => "Option<&'a str>".to_owned(),
            _ => String::new(),
        }
    }
    pub fn add_parameter(&mut self, name: &str, type_str: &str, bound_type: BoundType,
                         is_nullable: bool) -> bool {
        if self.used.iter().any(|ref n| n.0 == name) { return false; }
        if bound_type == BoundType::Into {
            if is_nullable == false { return true; }
            // For now, only one lifetime at a time is handled.
            if self.lifetimes.len() == 0 {
                self.lifetimes.push("a".into())
            }
        }
        let front = self.unused.pop_front();
        if let Some(alias) = front {
            self.used.push((name.into(), alias.clone(), type_str.into(), bound_type));
            true
        } else {
            false
        }
    }
    pub fn get_parameter_alias_info(&self, name: &str) -> Option<(&str, BoundType)> {
        self.used.iter().find(|ref n| n.0 == name)
            .map(|t| (&*t.1, t.3))
    }
    pub fn update_imports(&self, imports: &mut Imports) {
        //TODO: import with versions
        use self::BoundType::*;
        for used in &self.used {
            match used.3 {
                IsA => imports.add("glib::object::IsA", None),
                AsRef => imports.add_used_type(&used.2, None),
                Into => {}
            }
        }
    }
    pub fn is_empty(&self) -> bool {
        self.used.is_empty()
    }
    pub fn iter(&self) ->  Iter<(String, String, String, BoundType)> {
        self.used.iter()
    }
    pub fn iter_lifetimes(&self) -> Iter<String> {
        self.lifetimes.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_new_all() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::IsA;
        assert_eq!(bounds.add_parameter("a", "", typ), true);
        assert_eq!(bounds.add_parameter("a", "", typ), false);  //Don't add second time
        assert_eq!(bounds.add_parameter("b", "", typ), true);
        assert_eq!(bounds.add_parameter("c", "", typ), true);
        assert_eq!(bounds.add_parameter("d", "", typ), true);
        assert_eq!(bounds.add_parameter("e", "", typ), true);
        assert_eq!(bounds.add_parameter("f", "", typ), true);
        assert_eq!(bounds.add_parameter("g", "", typ), true);
        assert_eq!(bounds.add_parameter("h", "", typ), false);
    }

    #[test]
    fn get_parameter_alias_info() {
        let mut bounds: Bounds = Default::default();
        let typ = BoundType::IsA;
        bounds.add_parameter("a", "", typ);
        bounds.add_parameter("b", "", typ);
        assert_eq!(bounds.get_parameter_alias_info("a"), Some(("T", typ)));
        assert_eq!(bounds.get_parameter_alias_info("b"), Some(("U", typ)));
        assert_eq!(bounds.get_parameter_alias_info("c"), None);
    }
}
