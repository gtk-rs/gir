use analysis;
use analysis::properties::PropertyConversion;
use chunk::Chunk;

#[derive(Default)]
pub struct Builder {
    name: String,
    var_name: String,
    is_get: bool,
    is_child_property: bool,
    default_value: String,
    is_ref: bool,
    is_nullable: bool,
    is_into: bool,
    conversion: PropertyConversion,
}

#[cfg_attr(feature = "cargo-clippy", allow(wrong_self_convention))]
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

    pub fn var_name(&mut self, name: &str) -> &mut Builder {
        self.var_name = name.into();
        self
    }

    pub fn is_get(&mut self, value: bool) -> &mut Builder {
        self.is_get = value;
        self
    }

    pub fn default_value(&mut self, value: &str) -> &mut Builder {
        self.default_value = value.into();
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

    pub fn is_into(&mut self, is_into: bool) -> &mut Builder {
        self.is_into = is_into;
        self
    }

    pub fn conversion(&mut self, value: PropertyConversion) -> &mut Builder {
        self.conversion = value;
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
        use analysis::properties::PropertyConversion::*;
        let mut params = Vec::new();

        params.push(Chunk::Custom("self.to_glib_none().0".into()));
        if self.is_child_property {
            params.push(Chunk::Custom("item.to_glib_none().0".into()));
        }
        params.push(Chunk::Custom(format!("\"{}\".to_glib_none().0", self.name)));
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
            params: params,
        };
        body.push(Chunk::FfiCallConversion {
            ret: return_info,
            array_length_name: None,
            call: Box::new(ffi_call),
        });

        match self.conversion {
            AsI32 => {
                body.push(Chunk::Custom(
                    "from_glib(transmute(value.get::<i32>().unwrap()))".into(),
                ))
            }
            Bitflag => {
                body.push(Chunk::Custom(
                    "from_glib(transmute(value.get::<u32>().unwrap()))".into(),
                ))
            }
            _ => (),
        }

        let unsafe_ = Chunk::Unsafe(body);

        let mut chunks = Vec::new();

        let default_value_chunk = Chunk::Custom(format!("Value::from({})", self.default_value));
        chunks.push(Chunk::Let {
            name: "value".into(),
            is_mut: true,
            value: Box::new(default_value_chunk),
            type_: None,
        });
        chunks.push(unsafe_);

        if self.conversion == Direct {
            let unwrap = if self.is_nullable { "" } else { ".unwrap()" };
            chunks.push(Chunk::Custom(format!("value.get(){}", unwrap)));
        }

        chunks
    }

    fn chunks_for_set(&self) -> Vec<Chunk> {
        use analysis::properties::PropertyConversion::*;
        let mut params = Vec::new();

        params.push(Chunk::Custom("self.to_glib_none().0".into()));
        if self.is_child_property {
            params.push(Chunk::Custom("item.to_glib_none().0".into()));
        }
        params.push(Chunk::Custom(format!("\"{}\".to_glib_none().0", self.name)));
        let ref_str = if self.is_ref { "" } else { "&" };
        params.push(Chunk::Custom(format!(
            "Value::from({}{}).to_glib_none().0",
            ref_str,
            self.var_name
        )));

        let mut body = Vec::new();

        let ffi_call = Chunk::FfiCall {
            name: self.set_ffi_func(),
            params: params,
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

        let unsafe_ = Chunk::Unsafe(body);

        let mut chunks = Vec::new();

        if self.is_into {
            let value = Chunk::Custom(format!("{}.into()", self.var_name));
            chunks.push(Chunk::Let {
                name: self.var_name.clone(),
                is_mut: false,
                value: Box::new(value),
                type_: None,
            });
        }
        match self.conversion {
            AsI32 => {
                let value_chunk = Chunk::Custom(format!("{}.to_glib() as i32", self.var_name));
                chunks.push(Chunk::Let {
                    name: self.var_name.clone(),
                    is_mut: false,
                    value: Box::new(value_chunk),
                    type_: None,
                })
            }
            Bitflag => {
                let value_chunk =
                    Chunk::Custom(format!("{}.to_glib().bits() as u32", self.var_name));
                chunks.push(Chunk::Let {
                    name: self.var_name.clone(),
                    is_mut: false,
                    value: Box::new(value_chunk),
                    type_: None,
                })
            }
            _ => (),
        }

        chunks.push(unsafe_);

        chunks
    }

    fn get_ffi_func(&self) -> String {
        if self.is_child_property {
            "gtk_container_child_get_property".to_owned()
        } else {
            "gobject_ffi::g_object_get_property".to_owned()
        }
    }

    fn set_ffi_func(&self) -> String {
        if self.is_child_property {
            "gtk_container_child_set_property".to_owned()
        } else {
            "gobject_ffi::g_object_set_property".to_owned()
        }
    }
}
