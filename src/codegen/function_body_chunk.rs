use analysis::out_parameters::Mode;
use analysis::return_value;
use chunk::{chunks, Chunk};
use chunk::parameter_ffi_call_in;
use library;

#[derive(Clone)]
enum Parameter {
    In {
        parameter: parameter_ffi_call_in::Parameter,
        upcast: bool,
    },
    Out {
        name: String,
        prefix: String,
        suffix: String,
    },
}

use self::Parameter::*;

#[derive(Default)]
pub struct Builder {
    glib_name: String,
    parameters: Vec<Parameter>,
    ret: return_value::Info,
    outs_as_return: bool,
    outs_mode: Mode,
}

impl Builder {
    pub fn new() -> Builder {
        Default::default()
    }
    pub fn glib_name(&mut self, name: &str) -> &mut Builder {
        self.glib_name = name.into();
        self
    }
    pub fn ret(&mut self, ret: &return_value::Info) -> &mut Builder {
        self.ret = ret.clone();
        self
    }
    pub fn parameter(&mut self, parameter: &library::Parameter, upcast: bool) -> &mut Builder {
        self.parameters.push(Parameter::In {
            parameter: parameter.into(),
            upcast: upcast
        });
        self
    }
    pub fn out_parameter(&mut self, name: String, prefix: String, suffix: String) -> &mut Builder {
        self.parameters.push(Parameter::Out {
            name: name,
            prefix: prefix,
            suffix: suffix,
        });
        self.outs_as_return = true;
        self
    }
    pub fn outs_mode(&mut self, mode: Mode) -> &mut Builder {
        self.outs_mode = mode;
        self
    }
    // TODO: remove option
    pub fn generate(&self) -> Option<Chunk> {
        if self.outs_as_return {
            return None;
        }
        let mut body = Vec::new();

        let call = self.generate_call();
        let conv = self.generate_call_conversion(call);
        body.push(conv);

        let unsafe_ = Chunk::Unsafe(body);
        let block = Chunk::BlockHalf(chunks(unsafe_));
        Some(block)
    }
    fn generate_call(&self) -> Chunk {
        let params = self.generate_func_parameters();
        let func = Chunk::FfiCall {
            name: self.glib_name.clone(),
            params: params,
        };
        func
    }
    fn generate_call_conversion(&self, call: Chunk) -> Chunk {
        let conv = Chunk::FfiCallConversion {
            ret: self.ret.clone(),
            call: Box::new(call),
        };
        conv
    }
    fn generate_func_parameters(&self) -> Vec<Chunk> {
        let mut params = Vec::new();
        for par in &self.parameters {
            match *par {
                In { ref parameter, upcast } => {
                    let chunk = Chunk::FfiCallParameter{
                        par: parameter.clone(),
                        upcast: upcast,
                    };
                    params.push(chunk);
                }
                Out { .. } => (), //TODO: FfiCallOutParameter
            }
        }
        params
    }
}
