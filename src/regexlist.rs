use regex::*;
use std::iter::FromIterator;
use std::vec::Vec;

#[derive(Clone, Debug)]
pub struct RegexList(Vec<Regex>);

impl RegexList {
    pub fn new() -> RegexList {
        RegexList(Vec::new())
    }
    /// Returns true if and only if one of regex matches the string given.
    pub fn is_match(&self, text: &str) -> bool {
        for regex in &self.0 {
            if regex.is_match(text) {
                return true;
            }
        }
        false
    }
}

impl FromIterator<Regex> for RegexList {
    fn from_iter<T>(iterator: T) -> Self where T: IntoIterator<Item=Regex> {
        let vec = FromIterator::from_iter(iterator);
        RegexList(vec)
    }
}
