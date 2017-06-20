use analysis::conversion_type::ConversionType;
use analysis::parameter::Parameter as AnalysisParameter;
use analysis::out_parameters::Mode;
use analysis::ref_mode::RefMode;
use analysis::return_value;
use analysis::rust_type::rust_type;
use analysis::safety_assertion_mode::SafetyAssertionMode;
use chunk::{Chunk, TupleMode};
use chunk::parameter_ffi_call_in;
use chunk::parameter_ffi_call_out;
use env::Env;
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
    NullMutPtr,
}

use self::Parameter::*;

#[derive(Default)]
pub struct Builder {
    glib_name: String,
    parameters: Vec<Parameter>,
    ret: return_value::Info,
    outs_as_return: bool,
    outs_mode: Mode,
    assertion: SafetyAssertionMode,
}

impl Builder {
    pub fn new() -> Builder {
        Default::default()
    }
    pub fn glib_name(&mut self, name: &str) -> &mut Builder {
        self.glib_name = name.into();
        self
    }
    pub fn assertion(&mut self, assertion: SafetyAssertionMode) -> &mut Builder {
        self.assertion = assertion;
        self
    }
    pub fn ret(&mut self, ret: &return_value::Info) -> &mut Builder {
        self.ret = ret.clone();
        self
    }
    pub fn parameter(&mut self, parameter: &AnalysisParameter) -> &mut Builder {
        self.parameters.push(Parameter::In {
            parameter: parameter.into(),
        });
        self
    }
    pub fn out_parameter(&mut self, env: &Env, parameter: &AnalysisParameter) -> &mut Builder {
        use self::OutMemMode::*;
        let mem_mode = match ConversionType::of(&env.library, parameter.typ) {
            ConversionType::Pointer => {
                if parameter.caller_allocates {
                    UninitializedNamed(rust_type(env, parameter.typ).unwrap())
                } else {
                    use library::Type::*;
                    let type_ = env.library.type_(parameter.typ);
                    match *type_ {
                        Fundamental(fund) if fund == library::Fundamental::Utf8 || fund == library::Fundamental::Filename => {
                            if parameter.transfer == library::Transfer::Full {
                                NullMutPtr
                            } else {
                                NullPtr
                            }
                        },
                        _ => NullMutPtr,
                    }
                }
            }
            _ => Uninitialized,
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

        let mut chunks = Vec::new();
        self.add_into_conversion(&mut chunks);
        self.add_assertion(&mut chunks);
        chunks.push(unsafe_);
        Chunk::BlockHalf(chunks)
    }
    fn add_assertion(&self, chunks: &mut Vec<Chunk>) {
        match self.assertion {
            SafetyAssertionMode::None => (),
            SafetyAssertionMode::Skip =>
                chunks.insert(0, Chunk::AssertSkipInitialized),
            SafetyAssertionMode::InMainThread =>
                chunks.insert(0, Chunk::AssertInitializedAndInMainThread),
        }
    }
    fn add_into_conversion(&self, chunks: &mut Vec<Chunk>) {
        for param in self.parameters.iter().filter_map(|x| match *x {
            Parameter::In { ref parameter } if parameter.is_into => Some(parameter),
            _ => None,
        }) {
            let value = Chunk::Custom(format!("{}.into()", param.name));
            chunks.push(Chunk::Let {
                name: param.name.clone(),
                is_mut: false,
                value: Box::new(value),
                type_: None,
            });
            if param.ref_mode == RefMode::ByRef {
                let value = Chunk::Custom(format!("{}.to_glib_none()", param.name));
                chunks.push(Chunk::Let {
                    name: param.name.clone(),
                    is_mut: false,
                    value: Box::new(value),
                    type_: None,
                });
            }
        }
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
                In { ref parameter } => Chunk::FfiCallParameter {
                    par: parameter.clone(),
                },
                Out { ref parameter, .. } => Chunk::FfiCallOutParameter {
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
    fn get_outs_without_error(&self) -> Vec<&Parameter> {
        self.parameters.iter()
            .filter_map(|par| if let Out{ ref parameter, .. } = *par {
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
    fn write_out_variables(&self, v: &mut Vec<Chunk>) {
        let outs = self.get_outs();
        for par in outs {
            if let Out{ ref parameter, ref mem_mode } = *par {
                let val = self.get_uninitialized(mem_mode);
                let chunk = Chunk::Let{
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
            UninitializedNamed(ref name) => Chunk::UninitializedNamed{ name: name.clone() },
            NullPtr => Chunk::NullPtr,
            NullMutPtr => Chunk::NullMutPtr,
        }
    }
    fn generate_out_return(&self) -> Option<Chunk> {
        if !self.outs_as_return {
            return None;
        }
        let outs = self.get_outs_without_error();
        let mut chs: Vec<Chunk> = Vec::with_capacity(outs.len());
        for par in outs {
            if let Out{ ref parameter, ref mem_mode } = *par {
                chs.push(self.out_parameter_to_return(parameter, mem_mode));
            }
        }
        let chunk = Chunk::Tuple(chs, TupleMode::Auto);
        Some(chunk)
    }
    fn out_parameter_to_return(&self, parameter: &parameter_ffi_call_out::Parameter, mem_mode: &OutMemMode) -> Chunk {
        let value = Chunk::Custom(parameter.name.clone());
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
                    type_: Option::None,
                };
                let ret = ret.expect("No return in optional outs mode");
                let ret = Chunk::OptionalReturn{
                    condition: "ret".into(),
                    value: Box::new(ret),
                };
                (call, Some(ret))
            }
            Combined => {
                let call = Chunk::Let{
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
                let (boxed_call, ret_info) = if let Chunk::FfiCallConversion{call: inner, ret: ret_info} = call {
                    (inner, ret_info)
                } else {
                    panic!("Call without Chunk::FfiCallConversion")
                };
                let call = if use_ret {
                    Chunk::Let{
                        name: "ret".into(),
                        is_mut: false,
                        value: boxed_call,
                        type_: Option::None,
                    }
                } else {
                    Chunk::Let{
                        name: "_".into(),
                        is_mut: false,
                        value: boxed_call,
                        type_: Option::None,
                    }
                };
                let mut ret = ret.expect("No return in throws outs mode");
                if let Chunk::Tuple(ref mut vec, ref mut mode ) = ret {
                    *mode = TupleMode::WithUnit;
                    if use_ret {
                        let val = Chunk::Custom("ret".into());
                        let conv = Chunk::FfiCallConversion{
                            call: Box::new(val),
                            ret: ret_info,
                        };
                        vec.insert(0, conv);
                    }
                } else {
                    panic!("Return is not Tuple")
                }
                ret = Chunk::ErrorResultReturn{
                    value: Box::new(ret),
                };
                (call, Some(ret))
            }
        }
    }
}
