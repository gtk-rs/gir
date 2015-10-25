use analysis::out_parameters::Mode;
use chunk::{chunks, Chunk};

#[derive(Clone, Debug)]
enum Parameter {
    In {
        parameter: String,
    },
    Out {
        name: String,
        prefix: String,
        suffix: String,
    },
}

use self::Parameter::*;

#[derive(Default, Debug)]
pub struct Builder {
    glib_name: String,
    from_glib_prefix: String,
    from_glib_suffix: String,
    parameters: Vec<Parameter>,
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
    pub fn from_glib(&mut self, prefix_suffix: (String, String)) -> &mut Builder {
        self.from_glib_prefix = prefix_suffix.0;
        self.from_glib_suffix = prefix_suffix.1;
        self
    }
    pub fn parameter(&mut self, parameter: String) -> &mut Builder {
        self.parameters.push(Parameter::In { parameter: parameter });
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
        body.push(call);

        let unsafe_ = Chunk::Unsafe(body);
        let block = Chunk::BlockHalf(chunks(unsafe_));
        Some(block)
    }
    fn generate_call(&self) -> Chunk {
        let params = self.generate_func_parameters();
        let func = Chunk::FfiCall {
            name: self.glib_name.clone(),
            prefix: self.from_glib_prefix.clone(),
            suffix: self.from_glib_suffix.clone(),
            params: params,
        };
        func
    }
    fn generate_func_parameters(&self) -> Vec<Chunk> {
        let mut params = Vec::new();
        for par in &self.parameters {
            match *par {
                In { ref parameter } => {
                    let chunk = Chunk::FfiCallParameter(parameter.clone());
                    params.push(chunk);
                }
                Out { .. } => (), //TODO: FfiCallOutParameter
            }
        }
        params
    }
}
