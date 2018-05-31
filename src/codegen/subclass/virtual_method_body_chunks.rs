use analysis;
use analysis::conversion_type::ConversionType;
use analysis::function_parameters::CParameter as AnalysisCParameter;
use analysis::function_parameters::{Transformation, TransformationType};
use analysis::functions::{find_index_to_ignore, AsyncTrampoline};
use analysis::namespaces;
use analysis::out_parameters::Mode;
use analysis::return_value;
use analysis::rust_type::rust_type;
use analysis::safety_assertion_mode::SafetyAssertionMode;
use chunk::parameter_ffi_call_out;
use chunk::{Chunk, Param, TupleMode};
use codegen::function_body_chunk::Parameter::Out;
use env::Env;
use library::{self, ParameterDirection};
use nameutil;
use writer::ToCode;

use codegen::function_body_chunk::{c_type_mem_mode, Parameter, ReturnValue, OutMemMode};

#[derive(Default)]
pub struct Builder {
    object_name: String,
    object_class_c_type: String,
    ffi_crate_name: String,
    glib_name: String,
    method_name: String,
    parameters: Vec<Parameter>,
    transformations: Vec<Transformation>,
    ret: ReturnValue,
    outs_as_return: bool,
    outs_mode: Mode,
    assertion: SafetyAssertionMode,
}

impl Builder {
    pub fn new() -> Builder {
        Default::default()
    }

    pub fn object_name(&mut self, name: &str) -> &mut Builder {
        self.object_name = name.into();
        self
    }

    pub fn glib_name(&mut self, name: &str) -> &mut Builder {
        self.glib_name = name.into();
        self
    }

    pub fn object_class_c_type(&mut self, c_class_type: &str) -> &mut Builder {
        self.object_class_c_type = c_class_type.into();
        self
    }

    pub fn ffi_crate_name(&mut self, ns: &str) -> &mut Builder {
        self.ffi_crate_name = ns.into();
        self
    }

    pub fn method_name(&mut self, name: &str) -> &mut Builder {
        self.method_name = name.into();
        self
    }

    pub fn assertion(&mut self, assertion: SafetyAssertionMode) -> &mut Builder {
        self.assertion = assertion;
        self
    }

    pub fn ret(&mut self, ret: &return_value::Info) -> &mut Builder {
        self.ret = ReturnValue { ret: ret.clone() };
        self
    }

    pub fn parameter(&mut self) -> &mut Builder {
        self.parameters.push(Parameter::In);
        self
    }

    pub fn out_parameter(&mut self, env: &Env, parameter: &AnalysisCParameter) -> &mut Builder {
        let mem_mode = c_type_mem_mode(env, parameter);
        self.parameters.push(Parameter::Out {
            parameter: parameter_ffi_call_out::Parameter::new(parameter),
            mem_mode,
        });
        self.outs_as_return = true;
        self
    }

    pub fn transformations(&mut self, transformations: &[Transformation]) -> &mut Builder {
        self.transformations = transformations.to_owned();
        self
    }

    pub fn outs_mode(&mut self, mode: Mode) -> &mut Builder {
        self.outs_mode = mode;
        self
    }

    pub fn generate_default_impl(&self, env: &Env) -> Chunk {
        //TODO
        let mut chunks = Vec::new();
        Chunk::Chunks(chunks)
    }

    pub fn generate_base_impl(&self, env: &Env) -> Chunk {
        let mut body = Vec::new();

        body.push(self.let_klass());
        body.push(self.let_parent_klass());

        body.push(Chunk::Custom("(*parent_klass)".to_owned()));
        body.push(Chunk::Custom(format!(".{}", self.method_name).to_owned()));
        let mut args = Vec::new();
        args.push(self.base_impl_body_chunk());
        body.push(Chunk::Call {
            func_name: ".map".to_owned(),
            arguments: args,
            as_return: true,
        });




        //TODO: return variables?
        body.push(Chunk::Custom(".unwrap_or(())".to_owned()));


        let unsafe_ = Chunk::Unsafe(body);

        let mut chunks = Vec::new();

        self.add_into_conversion(&mut chunks);
        // self.add_assertion(&mut chunks);

        chunks.push(unsafe_);
        Chunk::Chunks(chunks)
    }

    pub fn generate_extern_c_func(&self, env: &Env) -> Chunk {
        let mut body = Vec::new();

        body.push(Chunk::Custom("callback_guard!();".to_owned()));
        body.push(Chunk::Custom("floating_reference_guard!(ptr);".to_owned()));

        body.push(Chunk::Let{ is_mut:false,
                                name: self.object_name.to_lowercase(),
                                value: Box::new(Chunk::Custom("&*(ptr as *mut T::InstanceStructType)".to_owned())),
                                type_: None
                             });

        body.push(Chunk::Let{ is_mut:false,
                                 name: "wrap".to_owned(),
                                 value: Box::new(Chunk::Custom("from_glib_borrow(ptr as *mut T::InstanceStructType)".to_owned())),
                                 type_: Some(Box::new(Chunk::Custom("T".to_owned())))
                              });

        body.push(Chunk::Let{ is_mut:false,
                                name: "imp".to_owned(),
                                value: Box::new(Chunk::Custom(format!("{}.get_impl()",
                            self.object_name.to_lowercase()).to_owned())),
                                type_: None
                             });



        body.push(Chunk::Custom(format!("imp.{}({})",
                                  self.method_name,
                                  &"").to_owned()));

        Chunk::Chunks(body)
    }

    fn base_impl_body_chunk(&self) -> Chunk {

        let mut body = Vec::new();

        if self.outs_as_return {
            self.write_out_variables(&mut body);
        }

        self.add_in_array_lengths(&mut body);

        let call = self.generate_ffi_call(Some("f".to_owned()));
        let call = self.generate_ffi_call_conversion(call);

        let ret = self.generate_out_return();
        let (call, ret) = self.apply_outs_mode(call, ret);

        body.push(call);
        if let Some(chunk) = ret {
            body.push(chunk);
        }

        Chunk::Closure {
            arguments: vec![Chunk::Custom("f".to_owned())],
            body: Box::new(Chunk::Chunks(body)
        ),
        }
    }

    pub fn generate_interface_init(&self, env: &Env, virtual_methods: &Vec<analysis::virtual_methods::Info>) -> Chunk {

        let mut body = Vec::new();

        let iface_name = format!("{}_iface", self.object_name.to_lowercase()).to_owned();
        let iface_get_type = format!("{}_get_type", self.object_name.to_lowercase()).to_owned();

        body.push(Chunk::Custom("callback_guard!();".to_owned()));
        body.push(Chunk::Let {
                    name: iface_name.clone(),
                    is_mut: false,
                    value: Box::new(Chunk::Custom(format!("&mut *(iface as *mut {}::{})",
                            self.ffi_crate_name, self.object_class_c_type
                        ).to_owned(),
                    )),
                    type_: None,
                });
        body.push(Chunk::Let {
                    name: "iface_type".to_owned(),
                    is_mut: false,
                    value: Box::new(Chunk::Custom("(*(iface as *const gobject_ffi::GTypeInterface)).g_type".to_owned(),
                    )),
                    type_: None,
                });
        body.push(Chunk::Let {
                    name: "type_".to_owned(),
                    is_mut: false,
                    value: Box::new(Chunk::Custom("(*(iface as *const gobject_ffi::GTypeInterface)).g_instance_type".to_owned(),
                    )),
                    type_: None,
                });

        body.push(Chunk::Let {
                    name: "klass".to_owned(),
                    is_mut: false,
                    value: Box::new(Chunk::Custom("&mut *(gobject_ffi::g_type_class_ref(type_) as *mut ClassStruct<T>)".to_owned(),
                    )),
                    type_: None,
                });

        body.push(Chunk::Let {
                    name: "interfaces_static".to_owned(),
                    is_mut: false,
                    value: Box::new(Chunk::Custom("&mut *(klass.interfaces_static as *mut Vec<_>)".to_owned(),
                    )),
                    type_: None,
                });

        body.push(Chunk::Custom("interfaces_static.push((iface_type, iface_data))".to_owned()));


        for method_analysis in virtual_methods {
            body.push(Chunk::Custom(
                format!("{iface}.{mname} = Some({type_fn}_{mname}::<T>);",
                    mname=method_analysis.name,
                    iface=iface_name,
                    type_fn=iface_get_type).to_owned()
            ));
        }


        Chunk::Chunks(body)

    }

    fn let_klass(&self) -> Chunk {
        Chunk::Let {
            name: "klass".to_owned(),
            is_mut: false,
            value: Box::new(Chunk::Custom("self.get_class()".to_owned())),
            type_: None,
        }
    }

    fn let_parent_klass(&self) -> Chunk {
        Chunk::Let {
            name: "parent_klass".to_owned(),
            is_mut: false,
            value: Box::new(Chunk::Cast {
                name: "(*klass).get_parent_class()".to_owned(),
                type_: format!(
                    "*const {}::{}",
                    self.ffi_crate_name, self.object_class_c_type
                ).to_owned(),
            }),
            type_: None,
        }
    }


    fn generate_func_parameters(&self) -> Vec<Chunk> {
        let mut params = Vec::new();
        for trans in &self.transformations {
            if !trans.transformation_type.is_to_glib() {
                continue;
            }
            let par = &self.parameters[trans.ind_c];
            let chunk = match par {
                In => Chunk::FfiCallParameter {
                    transformation_type: trans.transformation_type.clone(),
                },
                Out { ref parameter, .. } => Chunk::FfiCallOutParameter {
                    par: parameter.clone(),
                },
            };
            params.push(chunk);
        }
        params
    }

    fn write_out_variables(&self, v: &mut Vec<Chunk>) {
        let outs = self.get_outs();
        for par in outs {
            if let Out {
                ref parameter,
                ref mem_mode,
            } = *par
            {
                let val = self.get_uninitialized(mem_mode);
                let chunk = Chunk::Let {
                    name: parameter.name.clone(),
                    is_mut: true,
                    value: Box::new(val),
                    type_: None,
                };
                v.push(chunk);
            }
        }
    }
    fn get_uninitialized(&self, mem_mode: &OutMemMode) -> Chunk {
        use self::OutMemMode::*;
        match *mem_mode {
            Uninitialized => Chunk::Uninitialized,
            UninitializedNamed(ref name) => Chunk::UninitializedNamed { name: name.clone() },
            NullPtr => Chunk::NullPtr,
            NullMutPtr => Chunk::NullMutPtr,
        }
    }

    fn get_outs(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter_map(|par| if let Out { .. } = *par {
                Some(par)
            } else {
                None
            })
            .collect()
    }
    fn get_outs_without_error(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter_map(|par| if let Out { ref parameter, .. } = *par {
                if parameter.is_error {
                    None
                } else {
                    Some(par)
                }
            } else {
                None
            })
            .collect()
    }

    fn generate_ffi_call(&self, func_name: Option<String>) -> Chunk {
        let params = self.generate_func_parameters();
        let func = Chunk::Call {
            func_name: func_name.unwrap_or(self.glib_name.clone()),
            arguments: params,
            as_return: true,
        };
        func
    }
    fn generate_ffi_call_conversion(&self, call: Chunk) -> Chunk {
        Chunk::FfiCallConversion {
            ret: self.ret.ret.clone(),
            array_length_name: self.find_array_length_name(""),
            call: Box::new(call),
        }
    }
    fn find_array_length_name(&self, array_name_: &str) -> Option<String> {
        self.transformations
            .iter()
            .filter_map(|tr| {
                if let TransformationType::Length {
                    ref array_name,
                    ref array_length_name,
                    ..
                } = tr.transformation_type
                {
                    if array_name == array_name_ {
                        Some(array_length_name.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .next()
    }

    fn generate_out_return(&self) -> Option<Chunk> {
        if !self.outs_as_return {
            return None;
        }
        let outs = self.get_outs_without_error();
        let mut chs: Vec<Chunk> = Vec::with_capacity(outs.len());
        for par in outs {
            if let Out {
                ref parameter,
                ref mem_mode,
            } = *par
            {
                if self.transformations
                    .iter()
                    .any(|tr| match tr.transformation_type {
                        TransformationType::Length {
                            ref array_length_name,
                            ..
                        } if array_length_name == &parameter.name =>
                        {
                            true
                        }
                        _ => false,
                    }) {
                    continue;
                }

                chs.push(self.out_parameter_to_return(parameter, mem_mode));
            }
        }
        let chunk = Chunk::Tuple(chs, TupleMode::Auto);
        Some(chunk)
    }
    fn out_parameter_to_return(
        &self,
        parameter: &parameter_ffi_call_out::Parameter,
        mem_mode: &OutMemMode,
    ) -> Chunk {
        let value = Chunk::Custom(parameter.name.clone());
        if let OutMemMode::UninitializedNamed(_) = *mem_mode {
            value
        } else {
            Chunk::FromGlibConversion {
                mode: parameter.into(),
                array_length_name: self.find_array_length_name(&parameter.name),
                value: Box::new(value),
            }
        }
    }
    fn apply_outs_mode(&self, call: Chunk, ret: Option<Chunk>) -> (Chunk, Option<Chunk>) {
        use analysis::out_parameters::Mode::*;
        match self.outs_mode {
            None => (call, ret),
            Normal => (call, ret),
            Optional => {
                let call = Chunk::Let {
                    name: "ret".into(),
                    is_mut: false,
                    value: Box::new(call),
                    type_: Option::None,
                };
                let ret = ret.expect("No return in optional outs mode");
                let ret = Chunk::OptionalReturn {
                    condition: "ret".into(),
                    value: Box::new(ret),
                };
                (call, Some(ret))
            }
            Combined => {
                let call = Chunk::Let {
                    name: "ret".into(),
                    is_mut: false,
                    value: Box::new(call),
                    type_: Option::None,
                };
                let mut ret = ret.expect("No return in combined outs mode");
                if let Chunk::Tuple(ref mut vec, _) = ret {
                    vec.insert(0, Chunk::Custom("ret".into()));
                }
                (call, Some(ret))
            }
            Throws(use_ret) => {
                //extracting original FFI function call
                let (boxed_call, array_length_name, ret_info) = if let Chunk::FfiCallConversion {
                    call: inner,
                    array_length_name,
                    ret: ret_info,
                } = call
                {
                    (inner, array_length_name, ret_info)
                } else {
                    panic!("Call without Chunk::FfiCallConversion")
                };
                let call = if use_ret {
                    Chunk::Let {
                        name: "ret".into(),
                        is_mut: false,
                        value: boxed_call,
                        type_: Option::None,
                    }
                } else {
                    Chunk::Let {
                        name: "_".into(),
                        is_mut: false,
                        value: boxed_call,
                        type_: Option::None,
                    }
                };
                let mut ret = ret.expect("No return in throws outs mode");
                if let Chunk::Tuple(ref mut vec, ref mut mode) = ret {
                    *mode = TupleMode::WithUnit;
                    if use_ret {
                        let val = Chunk::Custom("ret".into());
                        let conv = Chunk::FfiCallConversion {
                            call: Box::new(val),
                            array_length_name,
                            ret: ret_info,
                        };
                        vec.insert(0, conv);
                    }
                } else {
                    panic!("Return is not Tuple")
                }
                ret = Chunk::ErrorResultReturn {
                    value: Box::new(ret),
                };
                (call, Some(ret))
            }
        }
    }

    fn add_into_conversion(&self, chunks: &mut Vec<Chunk>) {
        for trans in &self.transformations {
            if let TransformationType::Into {
                ref name,
                with_stash,
            } = trans.transformation_type
            {
                let value = Chunk::Custom(format!("{}.into()", name));
                chunks.push(Chunk::Let {
                    name: name.clone(),
                    is_mut: false,
                    value: Box::new(value),
                    type_: None,
                });
                if with_stash {
                    let value = Chunk::Custom(format!("{}.to_glib_none()", name));
                    chunks.push(Chunk::Let {
                        name: name.clone(),
                        is_mut: false,
                        value: Box::new(value),
                        type_: None,
                    });
                }
            }
        }
    }

    fn add_in_array_lengths(&self, chunks: &mut Vec<Chunk>) {
        for trans in &self.transformations {
            if let TransformationType::Length {
                ref array_name,
                ref array_length_name,
                ref array_length_type,
            } = trans.transformation_type
            {
                if let Parameter::In = self.parameters[trans.ind_c] {
                    let value =
                        Chunk::Custom(format!("{}.len() as {}", array_name, array_length_type));
                    chunks.push(Chunk::Let {
                        name: array_length_name.clone(),
                        is_mut: false,
                        value: Box::new(value),
                        type_: None,
                    });
                }
            }
        }
    }

}
