use crate::{analysis, chunk::Chunk};

#[derive(Default)]
pub struct Builder {
    name: String,
    in_trait: bool,
    var_name: String,
    is_get: bool,
    is_child_property: bool,
    type_: String,
    is_ref: bool,
    is_nullable: bool,
}

#[allow(clippy::wrong_self_convention)]
impl Builder {
    pub fn new() -> Builder {
        Default::default()
    }

    pub fn new_for_child_property() -> Builder {
        Builder {
            is_child_property: true,
            ..Default::default()
        }
    }

    pub fn name(&mut self, name: &str) -> &mut Builder {
        self.name = name.into();
        self
    }

    pub fn in_trait(&mut self, value: bool) -> &mut Builder {
        self.in_trait = value;
        self
    }

    pub fn var_name(&mut self, name: &str) -> &mut Builder {
        self.var_name = name.into();
        self
    }

    pub fn is_get(&mut self, value: bool) -> &mut Builder {
        self.is_get = value;
        self
    }

    pub fn type_(&mut self, type_: &str) -> &mut Builder {
        self.type_ = type_.into();
        self
    }

    pub fn is_ref(&mut self, value: bool) -> &mut Builder {
        self.is_ref = value;
        self
    }

    pub fn is_nullable(&mut self, value: bool) -> &mut Builder {
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
            "gtk_sys::GtkContainer"
        } else {
            "gobject_sys::GObject"
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

        let return_info = analysis::return_value::Info {
            parameter: None,
            base_tid: None,
            commented: false,
            bool_return_is_error: None,
        };
        let ffi_call = Chunk::FfiCall {
            name: self.get_ffi_func(),
            params,
        };

        body.push(Chunk::Let {
            name: "value".into(),
            is_mut: true,
            value: Box::new(Chunk::Custom(format!(
                "Value::from_type(<{} as StaticType>::static_type())",
                self.type_
            ))),
            type_: None,
        });

        body.push(Chunk::FfiCallConversion {
            ret: return_info,
            array_length_name: None,
            call: Box::new(ffi_call),
        });

        let unwrap = if self.is_nullable {
            // This one is strictly speaking nullable, but
            // we represent that with an empty Vec instead
            if self.type_ == "Vec<GString>" {
                ".unwrap()"
            } else {
                ""
            }
        } else {
            ".unwrap()"
        };
        body.push(Chunk::Custom(format!(
            "value.get().expect(\"Return Value for property `{}` getter\"){}",
            self.name, unwrap,
        )));

        let unsafe_ = Chunk::Unsafe(body);

        let mut chunks = Vec::new();
        chunks.push(unsafe_);

        chunks
    }

    fn chunks_for_set(&self) -> Vec<Chunk> {
        let mut params = Vec::new();

        let cast_target = if self.is_child_property {
            "gtk_sys::GtkContainer"
        } else {
            "gobject_sys::GObject"
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
        let ref_str = if self.is_ref { "" } else { "&" };
        params.push(Chunk::Custom(format!(
            "Value::from({}{}).to_glib_none().0",
            ref_str, self.var_name
        )));

        let mut body = Vec::new();

        let ffi_call = Chunk::FfiCall {
            name: self.set_ffi_func(),
            params,
        };
        let return_info = analysis::return_value::Info {
            parameter: None,
            base_tid: None,
            commented: false,
            bool_return_is_error: None,
        };
        body.push(Chunk::FfiCallConversion {
            ret: return_info,
            array_length_name: None,
            call: Box::new(ffi_call),
        });

        vec![Chunk::Unsafe(body)]
    }

    fn get_ffi_func(&self) -> String {
        if self.is_child_property {
            "gtk_sys::gtk_container_child_get_property".to_owned()
        } else {
            "gobject_sys::g_object_get_property".to_owned()
        }
    }

    fn set_ffi_func(&self) -> String {
        if self.is_child_property {
            "gtk_sys::gtk_container_child_set_property".to_owned()
        } else {
            "gobject_sys::g_object_set_property".to_owned()
        }
    }
}
