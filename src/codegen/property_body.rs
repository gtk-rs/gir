use crate::{
    analysis,
    chunk::Chunk,
    env::Env,
    nameutil::{use_glib_type, use_gtk_type},
};

pub struct Builder<'a> {
    name: String,
    in_trait: bool,
    var_name: String,
    is_get: bool,
    is_child_property: bool,
    type_: String,
    env: &'a Env,
}

#[allow(clippy::wrong_self_convention)]
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
            // TODO: make use of safe bindings for child properties setter
            self.child_property_setter()
        } else {
            let self_ = if self.in_trait {
                "self.as_ref()"
            } else {
                "self"
            };

            vec![Chunk::Custom(format!(
                "{}::property({}, \"{}\")",
                use_glib_type(self.env, "ObjectExt"),
                self_,
                self.name
            ))]
        }
    }

    fn chunks_for_set(&self) -> Vec<Chunk> {
        if self.is_child_property {
            // TODO: make use of safe bindings for child properties getter
            self.child_property_getter()
        } else {
            let self_ = if self.in_trait {
                "self.as_ref()"
            } else {
                "self"
            };

            vec![Chunk::Custom(format!(
                "{}::set_property({},\"{}\", &{})",
                use_glib_type(self.env, "ObjectExt"),
                self_,
                self.name,
                self.var_name
            ))]
        }
    }

    fn child_property_setter(&self) -> Vec<Chunk> {
        let mut body = Vec::new();

        let mut params = Vec::new();
        let cast_target = use_gtk_type(self.env, "ffi::GtkContainer");
        if self.in_trait {
            params.push(Chunk::Custom(format!(
                "self.to_glib_none().0 as *mut {}",
                cast_target
            )));
        } else {
            params.push(Chunk::Custom(format!(
                "self.as_ptr() as *mut {}",
                cast_target
            )));
        }
        params.push(Chunk::Custom("item.to_glib_none().0 as *mut _".into()));

        params.push(Chunk::Custom(format!(
            "b\"{}\\0\".as_ptr() as *const _",
            self.name
        )));
        params.push(Chunk::Custom("value.to_glib_none_mut().0".into()));

        let ffi_call = Chunk::FfiCall {
            name: use_gtk_type(self.env, "ffi::gtk_container_child_get_property"),
            params,
        };

        body.push(Chunk::Let {
            name: "value".into(),
            is_mut: true,
            value: Box::new(Chunk::Custom(format!(
                "glib::Value::from_type(<{} as StaticType>::static_type())",
                self.type_
            ))),
            type_: None,
        });

        body.push(Chunk::FfiCallConversion {
            ret: analysis::return_value::Info::default(),
            array_length_name: None,
            call: Box::new(ffi_call),
        });

        body.push(Chunk::Custom(format!(
            "value.get().expect(\"Return Value for property `{}` getter\")",
            self.name,
        )));
        vec![Chunk::Unsafe(body)]
    }

    fn child_property_getter(&self) -> Vec<Chunk> {
        let mut body = Vec::new();

        let mut params = Vec::new();
        let cast_target = use_gtk_type(self.env, "ffi::GtkContainer");
        if self.in_trait {
            params.push(Chunk::Custom(format!(
                "self.to_glib_none().0 as *mut {}",
                cast_target
            )));
        } else {
            params.push(Chunk::Custom(format!(
                "self.as_ptr() as *mut {}",
                cast_target
            )));
        }
        params.push(Chunk::Custom("item.to_glib_none().0 as *mut _".into()));
        params.push(Chunk::Custom(format!(
            "b\"{}\\0\".as_ptr() as *const _",
            self.name
        )));
        params.push(Chunk::Custom(format!(
            "{}.to_value().to_glib_none().0",
            self.var_name
        )));

        let ffi_call = Chunk::FfiCall {
            name: use_gtk_type(self.env, "ffi::gtk_container_child_set_property"),
            params,
        };
        body.push(Chunk::FfiCallConversion {
            ret: analysis::return_value::Info::default(),
            array_length_name: None,
            call: Box::new(ffi_call),
        });
        vec![Chunk::Unsafe(body)]
    }
}
