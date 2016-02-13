
macro_rules! identables {
    ( $items:ident < $item:ident >) => {
        #[derive(Clone, Debug)]
        pub struct $items(Vec<$item>);

        impl $items {
            #[allow(dead_code)]
            pub fn new() -> $items {
                $items(Vec::new())
            }

            pub fn parse(toml: Option<&Value>, object_name: &str) -> $items {
                let mut v = Vec::new();
                if let Some(pars) = toml.and_then(|val| val.as_slice()) {
                    for par in pars {
                        if let Some(par) = $item::parse(par, object_name) {
                            v.push(par);
                        }
                    }
                }

                $items(v)
            }

            pub fn matched(&self, name: &str) -> Vec<&$item> {
                self.0.iter().filter(|p| p.ident.is_match(name)).collect()
            }

            #[allow(dead_code)]
            #[cfg(test)]
            fn vec(&self) -> &Vec<$item> {
                &self.0
            }
        }
    };
}
