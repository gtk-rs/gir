use std::collections::vec_deque::VecDeque;
use std::slice::Iter;
use std::vec::Vec;

#[derive(Debug)]
pub struct Upcasts {
    unused: VecDeque<String>,
    //Vector tuples <parameter name>, <alias>, <type>
    used: Vec<(String, String, String)>,
}

impl Default for Upcasts {
    fn default () -> Upcasts {
        Upcasts {
            unused: "TUVWXYZ".chars().map(|ch| ch.to_string()).collect(),
            used: Vec::new(),
        }
    }
}

impl Upcasts {
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
        let mut upcasts: Upcasts = Default::default();
        assert_eq!(upcasts.add_parameter("a", ""), true);
        assert_eq!(upcasts.add_parameter("a", ""), false);  //Don't add second time
        assert_eq!(upcasts.add_parameter("b", ""), true);
        assert_eq!(upcasts.add_parameter("c", ""), true);
        assert_eq!(upcasts.add_parameter("d", ""), true);
        assert_eq!(upcasts.add_parameter("e", ""), true);
        assert_eq!(upcasts.add_parameter("f", ""), true);
        assert_eq!(upcasts.add_parameter("g", ""), true);
        assert_eq!(upcasts.add_parameter("h", ""), false);
    }

    #[test]
    fn get_parameter_type_alias() {
        let mut upcasts: Upcasts = Default::default();
        upcasts.add_parameter("a", "");
        upcasts.add_parameter("b", "");
        assert_eq!(upcasts.get_parameter_type_alias("a"), Some("T".into()));
        assert_eq!(upcasts.get_parameter_type_alias("b"), Some("U".into()));
        assert_eq!(upcasts.get_parameter_type_alias("c"), None);
    }
}
