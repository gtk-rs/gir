use crate::{chunk::Chunk, env::Env, nameutil::use_gtk_type};

pub struct Builder<'a> {
    name: String,
    in_trait: bool,
    var_name: String,
    is_get: bool,
    is_child_property: bool,
    type_: String,
    env: &'a Env,
}

impl<'a> Builder<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self {
            env,
            name: Default::default(),
            in_trait: Default::default(),
            var_name: Default::default(),
            is_get: Default::default(),
            is_child_property: Default::default(),
            type_: Default::default(),
        }
    }

    pub fn new_for_child_property(env: &'a Env) -> Self {
        Self {
            is_child_property: true,
            env,
            name: Default::default(),
            in_trait: Default::default(),
            var_name: Default::default(),
            is_get: Default::default(),
            type_: Default::default(),
        }
    }

    pub fn name(&mut self, name: &str) -> &mut Self {
        self.name = name.into();
        self
    }

    pub fn in_trait(&mut self, value: bool) -> &mut Self {
        self.in_trait = value;
        self
    }

    pub fn var_name(&mut self, name: &str) -> &mut Self {
        self.var_name = name.into();
        self
    }

    pub fn is_get(&mut self, value: bool) -> &mut Self {
        self.is_get = value;
        self
    }

    pub fn type_(&mut self, type_: &str) -> &mut Self {
        self.type_ = type_.into();
        self
    }

    pub fn generate(&self) -> Chunk {
        let chunks = if self.is_get {
            self.chunks_for_get()
        } else {
            self.chunks_for_set()
        };
        Chunk::BlockHalf(chunks)
    }

    fn chunks_for_get(&self) -> Vec<Chunk> {
        if self.is_child_property {
            let self_ = if self.in_trait {
                "self.as_ref()"
            } else {
                "self"
            };

            vec![Chunk::Custom(format!(
                "{}::child_property({}, &item.clone().upcast(),\"{}\")",
                use_gtk_type(self.env, "prelude::ContainerExtManual"),
                self_,
                self.name,
            ))]
        } else {
            let self_ = if self.in_trait {
                "self.as_ref()"
            } else {
                "self"
            };

            vec![Chunk::Custom(format!(
                "ObjectExt::property({}, \"{}\")",
                self_, self.name
            ))]
        }
    }

    fn chunks_for_set(&self) -> Vec<Chunk> {
        if self.is_child_property {
            let self_ = if self.in_trait {
                "self.as_ref()"
            } else {
                "self"
            };

            vec![Chunk::Custom(format!(
                "{}::child_set_property({}, &item.clone().upcast(),\"{}\", &{})",
                use_gtk_type(self.env, "prelude::ContainerExtManual"),
                self_,
                self.name,
                self.var_name
            ))]
        } else {
            let self_ = if self.in_trait {
                "self.as_ref()"
            } else {
                "self"
            };

            vec![Chunk::Custom(format!(
                "ObjectExt::set_property({},\"{}\", {})",
                self_, self.name, self.var_name
            ))]
        }
    }
}
