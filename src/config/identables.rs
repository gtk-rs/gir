use super::ident::Ident;
use toml::Value;

pub trait Parse : Sized {
    fn parse(toml: &Value, name: &str) -> Option<Self>;
}

pub trait Identables {
    type Item;

    fn parse(toml: Option<&Value>, object_name: &str) -> Vec<Self::Item>;
    fn matched(&self, name: &str) -> Vec<&Self::Item>;
}

impl<T: Parse + AsRef<Ident>> Identables for Vec<T> {
    type Item = T;
    
    fn parse(toml: Option<&Value>, object_name: &str) -> Vec<Self::Item> {
        let mut v = Vec::new();
        if let Some(configs) = toml.and_then(|val| val.as_slice()) {
            for config in configs {
                if let Some(item) = T::parse(config, object_name) {
                    v.push(item);
                }
            }
        }

        v
    }

    fn matched(&self, name: &str) -> Vec<&Self::Item> {
        self.iter().filter(|item| item.as_ref().is_match(name)).collect()
    }
}
