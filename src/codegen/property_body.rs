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
    is_ref: bool,
    is_nullable: bool,
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
            is_ref: Default::default(),
            is_nullable: Default::default(),
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
            is_ref: Default::default(),
            is_nullable: Default::default(),
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

    pub fn is_ref(&mut self, value: bool) -> &mut Self {
        self.is_ref = value;
        self
    }

    pub fn is_nullable(&mut self, value: bool) -> &mut Self {
        self.is_nullable = value;
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
        let mut params = Vec::new();

        let cast_target = if self.is_child_property {
            use_gtk_type(&self.env, "ffi::GtkContainer")
        } else {
            use_glib_type(&self.env, "gobject_ffi::GObject")
        };
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

        if self.is_child_property {
            params.push(Chunk::Custom("item.to_glib_none().0 as *mut _".into()));
        }
        params.push(Chunk::Custom(format!(
            "b\"{}\\0\".as_ptr() as *const _",
            self.name
        )));
        params.push(Chunk::Custom("value.to_glib_none_mut().0".into()));

        let mut body = Vec::new();

        let ffi_call = Chunk::FfiCall {
            name: self.get_ffi_func(),
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

    fn chunks_for_set(&self) -> Vec<Chunk> {
        let mut params = Vec::new();

        let cast_target = if self.is_child_property {
            use_gtk_type(&self.env, "ffi::GtkContainer")
        } else {
            use_glib_type(&self.env, "gobject_ffi::GObject")
        };
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

        if self.is_child_property {
            params.push(Chunk::Custom("item.to_glib_none().0 as *mut _".into()));
        }
        params.push(Chunk::Custom(format!(
            "b\"{}\\0\".as_ptr() as *const _",
            self.name
        )));
        params.push(Chunk::Custom(format!(
            "{}.to_value().to_glib_none().0",
            self.var_name
        )));

        let mut body = Vec::new();

        let ffi_call = Chunk::FfiCall {
            name: self.set_ffi_func(),
            params,
        };
        body.push(Chunk::FfiCallConversion {
            ret: analysis::return_value::Info::default(),
            array_length_name: None,
            call: Box::new(ffi_call),
        });

        vec![Chunk::Unsafe(body)]
    }

    fn get_ffi_func(&self) -> String {
        if self.is_child_property {
            use_gtk_type(&self.env, "ffi::gtk_container_child_get_property")
        } else {
            use_glib_type(&self.env, "gobject_ffi::g_object_get_property")
        }
    }

    fn set_ffi_func(&self) -> String {
        if self.is_child_property {
            use_gtk_type(&self.env, "ffi::gtk_container_child_set_property")
        } else {
            use_glib_type(&self.env, "gobject_ffi::g_object_set_property")
        }
    }
}
