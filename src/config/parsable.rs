use toml::Value;

pub trait Parse: Sized {
    fn parse(toml: &Value, name: &str) -> Option<Self>;
}

pub trait Parsable {
    type Item;

    fn parse(toml: Option<&Value>, object_name: &str) -> Vec<Self::Item>;
}

impl<T: Parse> Parsable for Vec<T> {
    type Item = T;

    fn parse(toml: Option<&Value>, object_name: &str) -> Vec<Self::Item> {
        let mut v = Vec::new();
        if let Some(configs) = toml.and_then(Value::as_array) {
            for config in configs {
                if let Some(item) = T::parse(config, object_name) {
                    v.push(item);
                }
            }
        }

        v
    }
}
