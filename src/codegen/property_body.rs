use crate::{analysis::bounds::Bound, chunk::Chunk, env::Env, nameutil::use_gtk_type};

#[derive(Debug, Default)]
pub struct Builder<'a> {
    name: String,
    in_trait: bool,
    var_name: String,
    for_get: bool,
    nullable: bool,
    is_child_property: bool,
    type_: String,
    set_bound: Option<&'a Bound>,
    env: Option<&'a Env>,
}

impl<'a> Builder<'a> {
    pub fn new(env: &'a Env, set_bound: Option<&'a Bound>) -> Self {
        Self {
            env: Some(env),
            set_bound,
            ..Default::default()
        }
    }

    pub fn new_for_child_property(env: &'a Env) -> Self {
        Self {
            is_child_property: true,
            env: Some(env),
            ..Default::default()
        }
    }

    pub fn name(mut self, name: &str) -> Self {
        self.name = name.into();
        self
    }

    pub fn in_trait(mut self, value: bool) -> Self {
        self.in_trait = value;
        self
    }

    pub fn var_name(mut self, name: &str) -> Self {
        self.var_name = name.into();
        self
    }

    pub fn for_get(mut self, value: bool) -> Self {
        self.for_get = value;
        self
    }

    pub fn nullable(mut self, nullable: bool) -> Self {
        self.nullable = nullable;
        self
    }

    pub fn type_(mut self, type_: &str) -> Self {
        self.type_ = type_.into();
        self
    }

    pub fn generate(&self) -> Chunk {
        let chunks = if self.for_get {
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
                use_gtk_type(self.env.unwrap(), "prelude::ContainerExtManual"),
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
                use_gtk_type(self.env.unwrap(), "prelude::ContainerExtManual"),
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

            let to_glib_extra = self.set_bound.as_ref().map_or_else(String::new, |b| {
                b.bound_type.get_to_glib_extra(self.nullable, false, false)
            });

            vec![Chunk::Custom(format!(
                "ObjectExt::set_property({self_},\"{}\", {}{to_glib_extra})",
                self.name, self.var_name,
            ))]
        }
    }
}
