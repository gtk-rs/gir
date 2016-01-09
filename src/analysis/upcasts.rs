use std::collections::vec_deque::VecDeque;
use std::slice::Iter;
use std::vec::Vec;

#[derive(Debug)]
pub struct Upcasts {
    unused: VecDeque<String>,
    //Vector tuples <parameter name>, <alias>, <type>, <with_default>
    used: Vec<(String, String, String, bool)>,
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
    pub fn add_parameter(&mut self, name: &str, type_str: &str, with_default: bool) -> bool {
        if self.used.iter().any(|ref n| n.0 == name)  { return false; }
        let front = self.unused.pop_front();
        if let Some(alias) = front {
            self.used.push((name.into(), alias.clone(), type_str.into(), with_default));
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
    pub fn iter(&self) ->  Iter<(String, String, String, bool)> {
        self.used.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_new_all() {
        let mut upcasts: Upcasts = Default::default();
        assert_eq!(upcasts.add_parameter("a", "", false), true);
        assert_eq!(upcasts.add_parameter("a", "", false), false);  //Don't add second time
        assert_eq!(upcasts.add_parameter("b", "", false), true);
        assert_eq!(upcasts.add_parameter("c", "", false), true);
        assert_eq!(upcasts.add_parameter("d", "", false), true);
        assert_eq!(upcasts.add_parameter("e", "", false), true);
        assert_eq!(upcasts.add_parameter("f", "", false), true);
        assert_eq!(upcasts.add_parameter("g", "", false), true);
        assert_eq!(upcasts.add_parameter("h", "", false), false);
    }

    #[test]
    fn get_parameter_type_alias() {
        let mut upcasts: Upcasts = Default::default();
        upcasts.add_parameter("a", "", false);
        upcasts.add_parameter("b", "", false);
        assert_eq!(upcasts.get_parameter_type_alias("a"), Some("T".into()));
        assert_eq!(upcasts.get_parameter_type_alias("b"), Some("U".into()));
        assert_eq!(upcasts.get_parameter_type_alias("c"), None);
    }
}
