use super::ident::Ident;

pub trait Matchable {
    type Item;

    fn matched(&self, name: &str) -> Vec<&Self::Item>;
}

impl<T: AsRef<Ident>> Matchable for [T] {
    type Item = T;

    fn matched(&self, name: &str) -> Vec<&Self::Item> {
        self.iter()
            .filter(|item| item.as_ref().is_match(name))
            .collect()
    }
}
