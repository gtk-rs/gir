use std::collections::vec_deque::VecDeque;
use std::slice::Iter;
use std::vec::Vec;

use library::{Library, Type, TypeId};

#[derive(Debug)]
pub struct Bounds {
    unused: VecDeque<String>,
    //Vector tuples <parameter name>, <alias>, <type>
    used: Vec<(String, String, String)>,
}

impl Default for Bounds {
    fn default () -> Bounds {
        Bounds {
            unused: "TUVWXYZ".chars().map(|ch| ch.to_string()).collect(),
            used: Vec::new(),
        }
    }
}

impl Bounds {
    pub fn is_needed(library: &Library, type_id: TypeId) -> bool {
        match *library.type_(type_id) {
            Type::Class(ref klass) => !klass.children.is_empty(),
            Type::Interface(..) => true,
            _ => false,
        }
    }
    pub fn add_parameter(&mut self, name: &str, type_str: &str) -> bool {
        if self.used.iter().any(|ref n| n.0 == name)  { return false; }
        let front = self.unused.pop_front();
        if let Some(alias) = front {
            self.used.push((name.into(), alias.clone(), type_str.into()));
            true
        } else {
            false
        }
    }
    pub fn get_parameter_type_alias(&self, name: &str) -> Option<String> {
        self.used.iter().find(|ref n| n.0 == name)
            .map(|t| t.1.clone())
    }
    pub fn is_empty(&self) -> bool {
        self.used.is_empty()
    }
    pub fn iter(&self) ->  Iter<(String, String, String)> {
        self.used.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_new_all() {
        let mut bounds: Bounds = Default::default();
        assert_eq!(bounds.add_parameter("a", ""), true);
        assert_eq!(bounds.add_parameter("a", ""), false);  //Don't add second time
        assert_eq!(bounds.add_parameter("b", ""), true);
        assert_eq!(bounds.add_parameter("c", ""), true);
        assert_eq!(bounds.add_parameter("d", ""), true);
        assert_eq!(bounds.add_parameter("e", ""), true);
        assert_eq!(bounds.add_parameter("f", ""), true);
        assert_eq!(bounds.add_parameter("g", ""), true);
        assert_eq!(bounds.add_parameter("h", ""), false);
    }

    #[test]
    fn get_parameter_type_alias() {
        let mut bounds: Bounds = Default::default();
        bounds.add_parameter("a", "");
        bounds.add_parameter("b", "");
        assert_eq!(bounds.get_parameter_type_alias("a"), Some("T".into()));
        assert_eq!(bounds.get_parameter_type_alias("b"), Some("U".into()));
        assert_eq!(bounds.get_parameter_type_alias("c"), None);
    }
}
