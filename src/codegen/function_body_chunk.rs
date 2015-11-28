use analysis::conversion_type::ConversionType;
use analysis::out_parameters::Mode;
use analysis::return_value;
use chunk::{chunks, Chunk};
use chunk::parameter_ffi_call_in;
use chunk::parameter_ffi_call_out;
use library;

#[derive(Clone)]
enum Parameter {
    In {
        parameter: parameter_ffi_call_in::Parameter,
    },
    Out {
        parameter: parameter_ffi_call_out::Parameter,
        mem_mode: OutMemMode,
    },
}

#[derive(Clone, Eq, PartialEq)]
enum OutMemMode {
    Uninitialized,
    UninitializedNamed(String),
    NullPtr,
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
    pub fn parameter(&mut self, parameter: &library::Parameter) -> &mut Builder {
        self.parameters.push(Parameter::In {
            parameter: parameter.into(),
        });
        self
    }
    pub fn out_parameter(&mut self, library: &library::Library, parameter: &library::Parameter) -> &mut Builder {
        use self::OutMemMode::*;
        let mem_mode = if ConversionType::of(library, parameter.typ) == ConversionType::Pointer {
            if parameter.caller_allocates {
                let type_name = library.type_(parameter.typ).get_name();
                UninitializedNamed(type_name.clone())
            } else {
                NullPtr
            }
        } else {
            Uninitialized
        };
        self.parameters.push(Parameter::Out {
            parameter: parameter.into(),
            mem_mode: mem_mode,
        });
        self.outs_as_return = true;
        self
    }
    pub fn outs_mode(&mut self, mode: Mode) -> &mut Builder {
        self.outs_mode = mode;
        self
    }
    pub fn generate(&self) -> Chunk {
        let mut body = Vec::new();

        if self.outs_as_return {
            self.write_out_variables(&mut body);
        }

        let call = self.generate_call();
        let call = self.generate_call_conversion(call);
        let ret = self.generate_out_return();
        let (call, ret) = self.apply_outs_mode(call, ret);

        body.push(call);
        if let Some(chunk) = ret {
            body.push(chunk);
        }

        let unsafe_ = Chunk::Unsafe(body);
        Chunk::BlockHalf(chunks(unsafe_))
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
            let chunk = match *par {
                In { ref parameter } => Chunk::FfiCallParameter{
                    par: parameter.clone(),
                },
                Out { ref parameter, .. } => Chunk::FfiCallOutParameter{
                    par: parameter.clone(),
                },
            };
            params.push(chunk);
        }
        params
    }
    fn get_outs(&self) -> Vec<&Parameter> {
        self.parameters.iter()
            .filter_map(|par| if let Out{ .. } = *par { Some(par) } else { None })
            .collect()
    }
    fn write_out_variables(&self, v: &mut Vec<Chunk>) {
        let outs = self.get_outs();
        for par in outs {
            if let Out{ ref parameter, ref mem_mode } = *par {
                let val = self.get_uninitialized(mem_mode);
                let chunk = Chunk::Let{
                    name: parameter.name.clone(),
                    is_mut: true,
                    value: Box::new(val),
                };
                v.push(chunk);
            }
        }
    }
    fn get_uninitialized(&self, mem_mode: &OutMemMode) -> Chunk {
        use self::OutMemMode::*;
        match *mem_mode {
            Uninitialized => Chunk::Uninitialized,
            UninitializedNamed(ref name) => Chunk::UninitializedNamed{ name: name.clone() },
            NullPtr => Chunk::NullMutPtr,
        }
    }
    fn generate_out_return(&self) -> Option<Chunk> {
        if !self.outs_as_return {
            return None;
        }
        let outs = self.get_outs();
        let chunk = if outs.len() == 1 {
            if let Out{ ref parameter, ref mem_mode } = *(outs[0]) {
                self.out_parameter_to_return(parameter, mem_mode)
            } else { unreachable!() } 
        } else {
            let mut chs: Vec<Chunk> = Vec::new();
            for par in outs {
                if let Out{ ref parameter, ref mem_mode } = *par {
                    chs.push(self.out_parameter_to_return(parameter, mem_mode));
                }
            }
            Chunk::Tuple(chs)
        };
        Some(chunk)
    }
    fn out_parameter_to_return(&self, parameter: &parameter_ffi_call_out::Parameter, mem_mode: &OutMemMode) -> Chunk {
        let value = Chunk::VariableValue{name: parameter.name.clone()};
        if let OutMemMode::UninitializedNamed(_) = *mem_mode {
            value
        } else {
            Chunk::FromGlibConversion{
                mode: parameter.into(),
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
                let call = Chunk::Let{
                    name: "ret".into(),
                    is_mut: false,
                    value: Box::new(call),
                };
                let ret = ret.expect("No return in optional outs mode");
                let ret = Chunk::OptionalReturn{
                    condition: "ret".into(),
                    value: Box::new(ret),
                };
                (call, Some(ret))
            },
        }
    }
}
