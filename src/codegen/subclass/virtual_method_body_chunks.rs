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

use codegen::function_body_chunk::{c_type_mem_mode, Parameter, ReturnValue};
use codegen::parameter::*;

#[derive(Default)]
pub struct Builder {
    object_name: String,
    object_class_c_type: String,
    ffi_crate_name: String,
    method_name: String,
    parameters: Vec<Parameter>,
    transformations: Vec<Transformation>,
    ret: ReturnValue,
    outs_as_return: bool,
    outs_mode: Mode,
}

impl Builder {
    pub fn new() -> Builder {
        Default::default()
    }

    pub fn object_name(&mut self, name: &str) -> &mut Builder {
        self.object_name = name.into();
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
        chunks.push(unsafe_);
        Chunk::Chunks(chunks)
    }

    pub fn generate_extern_c_func(&self, env: &Env) -> Chunk {
        let mut chunks = Vec::new();

        chunks.push(Chunk::Custom("callback_guard!();".to_owned()));
        chunks.push(Chunk::Custom("floating_reference_guard!(ptr);".to_owned()));

        chunks.push(Chunk::Let{ is_mut:false,
                                name: self.object_name.to_lowercase(),
                                value: Box::new(Chunk::Custom("&*(ptr as *mut T::InstanceStructType)".to_owned())),
                                type_: None
                             });

         chunks.push(Chunk::Let{ is_mut:false,
                                 name: "wrap".to_owned(),
                                 value: Box::new(Chunk::Custom("from_glib_borrow(ptr as *mut T::InstanceStructType)".to_owned())),
                                 type_: Some(Box::new(Chunk::Custom("T".to_owned())))
                              });

        chunks.push(Chunk::Let{ is_mut:false,
                                name: "imp".to_owned(),
                                value: Box::new(Chunk::Custom(format!("{}.get_impl()",
                            self.object_name.to_lowercase()).to_owned())),
                                type_: None
                             });

        chunks.push(Chunk::Custom(format!("imp.{}({})",
                                  self.method_name,
                                  &"").to_owned()));

        Chunk::Chunks(chunks)
    }



    fn base_impl_body_chunk(&self) -> Chunk {
        Chunk::Closure {
            arguments: vec![Chunk::Custom("f".to_owned())],
            body: Box::new(Chunk::Call {
                func_name: "f".to_owned(),
                arguments: self.generate_func_parameters(),
                as_return: true,
            }),
        }
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
}
