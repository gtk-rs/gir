use analysis::conversion_type::ConversionType;
use analysis::functions::{AsyncTrampoline, find_index_to_ignore};
use analysis::function_parameters::CParameter as AnalysisCParameter;
use analysis::function_parameters::{Transformation, TransformationType};
use analysis::trampolines::Trampoline;
use analysis::out_parameters::Mode;
use analysis::namespaces;
use analysis::return_value;
use analysis::rust_type::rust_type;
use analysis::safety_assertion_mode::SafetyAssertionMode;
use chunk::{Chunk, Param, TupleMode};
use chunk::parameter_ffi_call_out;
use env::Env;
use library::{self, ParameterDirection};
use nameutil;
use std::collections::HashMap;

#[derive(Clone, Debug)]
enum Parameter {
    //Used to separate in and out parameters in `add_in_array_lengths`
    // and `generate_func_parameters`
    In,
    Out {
        parameter: parameter_ffi_call_out::Parameter,
        mem_mode: OutMemMode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum OutMemMode {
    Uninitialized,
    UninitializedNamed(String),
    NullPtr,
    NullMutPtr,
}

#[derive(Clone, Default)]
struct ReturnValue {
    pub ret: return_value::Info,
}

use self::Parameter::*;

#[derive(Default)]
pub struct Builder {
    async_trampoline: Option<AsyncTrampoline>,
    callbacks: Vec<Trampoline>,
    destroys: Vec<Trampoline>,
    glib_name: String,
    parameters: Vec<Parameter>,
    transformations: Vec<Transformation>,
    ret: ReturnValue,
    outs_as_return: bool,
    outs_mode: Mode,
    assertion: SafetyAssertionMode,
    remove_params: Option<Vec<usize>>,
}

impl Builder {
    pub fn new() -> Builder {
        Default::default()
    }
    pub fn async_trampoline(&mut self, trampoline: &AsyncTrampoline) -> &mut Builder {
        self.async_trampoline = Some(trampoline.clone());
        self
    }
    pub fn callback(&mut self, trampoline: &Trampoline) -> &mut Builder {
        self.callbacks.push(trampoline.clone());
        self
    }
    pub fn destroy(&mut self, trampoline: &Trampoline) -> &mut Builder {
        self.destroys.push(trampoline.clone());
        self
    }
    pub fn remove_params(&mut self, remove_params: &[usize]) -> &mut Builder {
        self.remove_params = Some(remove_params.to_owned());
        self
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
    pub fn generate(&self, env: &Env, bounds: String, bounds_names: String) -> Chunk {
        let mut body = Vec::new();

        if self.outs_as_return {
            self.write_out_variables(&mut body);
        }

        // Key: user data index
        // Value: (global position used as id, type, callbacks)
        let mut group_by_user_data: HashMap<usize, (usize, Option<String>, Vec<&Trampoline>)> =
            HashMap::new();

        // We group arguments by callbacks.
        if !self.callbacks.is_empty() || !self.destroys.is_empty() {
            for (pos, callback) in self.callbacks.iter().enumerate() {
                let user_data_index = callback.user_data_index;
                if group_by_user_data.get(&user_data_index).is_some() {
                    continue;
                }
                let calls = self.callbacks.iter()
                                          .filter(|c| c.user_data_index == user_data_index)
                                          .collect::<Vec<_>>();
                group_by_user_data.insert(
                    user_data_index,
                    (pos,
                    if calls.len() > 1 {
                        Some(format!("Box_<Box_<({})>>",
                                     calls.iter()
                                          .map(|c| if c.is_call {
                                              format!("Option<{}>", c.bound_name)
                                          } else {
                                              format!("&{}", c.bound_name)
                                          })
                                          .collect::<Vec<_>>()
                                          .join(", ")))
                    } else {
                        None
                    },
                    calls)
                );
            }
        }

        let call = self.generate_call(&group_by_user_data);
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
        self.add_in_array_lengths(&mut chunks);
        self.add_assertion(&mut chunks);

        if !self.callbacks.is_empty() || !self.destroys.is_empty() {
            // Key: user data index
            // Value: the current pos in the tuple for the given argument.
            let mut poses = HashMap::with_capacity(group_by_user_data.len());
            for trampoline in self.callbacks.iter() {
                let user_data_index = trampoline.user_data_index;
                let pos = poses.entry(user_data_index).or_insert_with(|| 0);
                self.add_trampoline(env,
                                    &mut chunks,
                                    trampoline,
                                    &group_by_user_data.get(&user_data_index).unwrap().1,
                                    *pos,
                                    &bounds,
                                    &bounds_names,
                                    false);
                *pos += 1;
            }
            for destroy in self.destroys.iter() {
                self.add_trampoline(env,
                                    &mut chunks,
                                    destroy,
                                    &group_by_user_data.get(&destroy.user_data_index).unwrap().1,
                                    0, // doesn't matter for destroy
                                    &bounds,
                                    &bounds_names,
                                    true);
            }
            for (_, (pos, full_type, ref calls)) in group_by_user_data.iter() {
                if calls.len() > 1 {
                    chunks.push(
                        Chunk::Let {
                            name: format!("super_callback{}", pos),
                            is_mut: false,
                            value: Box::new(
                                Chunk::Custom(
                                    format!("Box_::new(Box_::new(({})))",
                                            calls.iter()
                                                 .map(|c| format!("{}_data", c.name))
                                                 .collect::<Vec<_>>()
                                                 .join(", ")))),
                            type_: Some(Box::new(Chunk::Custom(full_type.clone().unwrap()))),
                        }
                    );
                } else if !calls.is_empty() {
                    chunks.push(Chunk::Let {
                        name: format!("super_callback{}", pos),
                        is_mut: false,
                        value: Box::new(Chunk::Custom(format!("{}{}_data",
                                                              if calls[0].is_call { "&" } else { "" },
                                                              calls[0].name))),
                        type_: Some(Box::new(Chunk::Custom(
                            if calls[0].is_call {
                                format!("&Option<{}>", calls[0].bound_name)
                            } else {
                                format!("Box_<Box_<Option<{}>>>", calls[0].bound_name)
                            }))),
                    });
                }
            }
        } else if let Some(ref trampoline) = self.async_trampoline {
            self.add_async_trampoline(env, &mut chunks, trampoline);
        }
        chunks.push(unsafe_);
        Chunk::BlockHalf(chunks)
    }

    fn add_trampoline(&self, env: &Env, chunks: &mut Vec<Chunk>, trampoline: &Trampoline,
                      full_type: &Option<String>, pos: usize, bounds: &str, bounds_names: &str,
                      is_destroy: bool) {
        if full_type.is_none() && !is_destroy {
            if trampoline.is_call {
                chunks.push(
                    Chunk::Custom(
                        format!("let {0}_data: Option<{1}> = {0}.into();",
                                trampoline.name, trampoline.bound_name)));
            } else {
                chunks.push(
                    Chunk::Custom(
                        format!("let {0}_data: Box_<Box_<Option<{1}>>> = Box::new(Box::new({0}.into()));",
                                trampoline.name, trampoline.bound_name)));
            }
        } else if !is_destroy {
            if trampoline.is_call {
                chunks.push(Chunk::Custom(format!("let {0}_data: &{1} = &{0};",
                                                  trampoline.name,
                                                  trampoline.bound_name)));
            } else {
                chunks.push(Chunk::Custom(format!("let {0}_data: Option<{1}> = {0}.into();",
                                                  trampoline.name,
                                                  trampoline.bound_name)));
            }
        }

        let mut body = Vec::new();
        let mut arguments = Vec::new();

        for par in trampoline.parameters.transformations.iter() {
            if trampoline.parameters.c_parameters[par.ind_c].c_type == "gpointer" ||
               par.name == "this" {
                continue;
            }
            let type_ = env.type_(par.typ);
            match *type_ {
                library::Type::Fundamental(ref x) if !x.requires_conversion() => {}
                library::Type::Fundamental(library::Fundamental::Boolean) |
                library::Type::Fundamental(library::Fundamental::UniChar) => {
                    body.push(Chunk::Custom(format!("let {0} = from_glib({0});", par.name)));
                }
                _ => {
                    let (begin, end) = ::codegen::translate_from_glib::from_glib_xxx(par.transfer, None);
                    body.push(Chunk::Custom(format!("let {1} = {0}{1}{2};", begin, par.name, end)));
                }
            }
            arguments.push(Chunk::Name(par.name.clone()));
        }

        let func = trampoline.parameters
                             .c_parameters
                             .last()
                             .map(|p| p.name.clone())
                             .unwrap_or_else(|| "Unknown".to_owned());

        if let Some(ref full_type) = full_type {
            body.push(
                Chunk::Let {
                    name: format!("{}callback", if is_destroy { "_" } else { "" }),
                    is_mut: false,
                    value: Box::new(Chunk::Custom(format!("Box_::from_raw({} as *mut _)", func))),
                    type_: Some(Box::new(Chunk::Custom(full_type.clone()))),
                }
            );
            if !is_destroy {
                if !trampoline.is_call {
                    body.push(
                        Chunk::Custom(
                            format!("{}if let Some(ref callback) = callback.{} {{",
                                    if trampoline.ret.c_type != "void" { "let res = " } else { "" },
                                    pos)));
                } else if trampoline.ret.c_type != "void" {
                    body.push(Chunk::Custom("let res = ".to_owned()));
                }
            }
        } else {
            body.push(
                Chunk::Let {
                    name: format!("{}callback", if is_destroy { "_" } else { "" }),
                    is_mut: false,
                    value: Box::new(Chunk::Custom(
                        if trampoline.is_call {
                            format!("{} as *const _ as usize as *mut {}", func, self.callbacks[0].bound_name)
                        } else {
                            format!("Box_::from_raw({} as *mut _)", func)
                        })),
                    type_: Some(Box::new(Chunk::Custom(
                        if trampoline.is_call {
                            format!("*mut {}", self.callbacks[0].bound_name)
                        } else {
                            format!("Box_<Box_<Option<{}>>>", self.callbacks[0].bound_name)
                        }))),
                }
            );
            if !is_destroy {
                if !trampoline.is_call {
                    body.push(Chunk::Custom(format!("{}if let Some(ref callback) = **callback {{",
                                                    if trampoline.ret.c_type != "void" { "let res = " } else { "" })));
                } else if trampoline.ret.c_type != "void" {
                    body.push(Chunk::Custom("let res = ".to_owned()));
                }
            }
        }
        if !is_destroy {
            use writer::to_code::ToCode;
            body.push(Chunk::Custom(format!("{}({}){}",
                                            if trampoline.is_call { "(*callback)" } else { "\tcallback"},
                                            arguments.iter()
                                                     .flat_map(|arg| arg.to_code(env))
                                                     .collect::<Vec<_>>()
                                                     .join(", "),
                                            if trampoline.is_call { ";" } else { "" },)));
            if !trampoline.is_call {
                body.push(Chunk::Custom("} else {".to_owned()));
                body.push(Chunk::Custom("\tpanic!(\"cannot get closure...\")".to_owned()));
                body.push(Chunk::Custom("};".to_owned()));
                if full_type.is_some() {
                    body.push(Chunk::Custom("Box_::into_raw(callback);".to_owned()));
                }
            }
            if trampoline.ret.c_type != "void" {
                use ::analysis::conversion_type::ConversionType;

                body.push(Chunk::Custom(match ConversionType::of(env, trampoline.ret.typ) {
                    ConversionType::Direct => "res".to_owned(),
                    ConversionType::Scalar | ConversionType::Pointer => {
                        if rust_type(env, trampoline.ret.typ).unwrap() != "GString" {
                            "res.to_glib()".to_owned()
                        } else {
                            "res.to_glib_full()".to_owned()
                        }
                    }
                    ConversionType::Borrow => panic!("cannot return borrowed type..."),
                    ConversionType::Unknown => "res".to_owned(),
                }));
            }
        }

        let extern_func = Chunk::ExternCFunc {
            name: format!("{}_func", trampoline.name),
            parameters: trampoline.parameters
                                  .c_parameters.iter()
                                  .skip(1) // to skip the generated this
                                  .map(|p| {
                                      if p.c_type == "gpointer" {
                                          Param { name: p.name.clone(),
                                                  typ: "glib_ffi::gpointer".to_owned() }
                                      } else {
                                          Param { name: p.name.clone(),
                                                  typ: ::analysis::ffi_type::ffi_type(env, p.typ, &p.c_type).expect("failed to write c_type") }
                                      }
                                  })
                                  .collect::<Vec<_>>(),
            body: Box::new(Chunk::Chunks(body)),
            return_value: if trampoline.ret.c_type != "void" {
                let p = &trampoline.ret;
                Some(if p.c_type == "gpointer" {
                    "glib_ffi::gpointer".to_owned()
                } else {
                    ::analysis::ffi_type::ffi_type(env, p.typ, &p.c_type).expect("failed to write c_type")
                })
            } else {
                None
            },
            bounds: bounds.to_owned(),
        };

        chunks.push(extern_func);
        if !is_destroy {
            if !trampoline.is_call {
                chunks.push(Chunk::Custom(format!("let {0} = if {0}_data.is_some() {{ Some({0}_func::<{1}> as _) }} else {{ None }};",
                                                  trampoline.name,
                                                  bounds_names)));
            } else {
                chunks.push(Chunk::Custom(format!("let {0} = Some({0}_func::<{1}> as _);",
                                                  trampoline.name,
                                                  bounds_names)));
            }
        } else {
            chunks.push(
                Chunk::Custom(
                    format!("let destroy_call{} = Some({}_func::<{}> as _);",
                            trampoline.destroy_index,
                            trampoline.name,
                            bounds_names)));
        }
    }

    fn add_async_trampoline(&self, env: &Env, chunks: &mut Vec<Chunk>, trampoline: &AsyncTrampoline) {
        chunks.push(Chunk::BoxFn {
            name: None,
            typ: format!("{}", trampoline.bound_name),
        });

        let mut finish_args = vec![];
        if trampoline.is_method {
            finish_args.push(Chunk::Cast {
                name: "_source_object".to_string(),
                type_: "*mut _".to_string(),
            });
        }
        finish_args.push(Chunk::Name("res".to_string()));
        finish_args.extend(trampoline.output_params.iter()
                           .filter(|param| param.direction == ParameterDirection::Out)
                           .map(|param| Chunk::FfiCallOutParameter{ par: param.into() }));
        let index_to_ignore = find_index_to_ignore(&trampoline.output_params);
        let mut result: Vec<_> = trampoline.output_params.iter().enumerate()
            .filter(|&(index, param)| param.direction == ParameterDirection::Out && param.name != "error" &&
                    Some(index) != index_to_ignore)
            .map(|(_, param)| {
                let value = Chunk::Custom(param.name.clone());
                let mem_mode = c_type_mem_mode_lib(env, param.typ, param.caller_allocates, param.transfer);
                if let OutMemMode::UninitializedNamed(_) = mem_mode {
                    value
                } else {
                    Chunk::FromGlibConversion {
                        mode: param.into(),
                        array_length_name: self.array_length(param).cloned(),
                        value: Box::new(value),
                    }
                }
            }).collect();

        if let Some(ref ffi_ret) = trampoline.ffi_ret {
            let mem_mode = c_type_mem_mode_lib(env, ffi_ret.typ, ffi_ret.caller_allocates, ffi_ret.transfer);
            let value = Chunk::Name("ret".to_string());
            if let OutMemMode::UninitializedNamed(_) = mem_mode {
                result.insert(0, value);
            } else {
                result.insert(0,
                    Chunk::FromGlibConversion {
                        mode: ffi_ret.into(),
                        array_length_name: self.array_length(ffi_ret).cloned(),
                        value: Box::new(value),
                    });
            }
        }

        let result = Chunk::Tuple(result, TupleMode::WithUnit);
        let gio_crate_name = crate_name("Gio", env);
        let gobject_crate_name = crate_name("GObject", env);
        let glib_crate_name = crate_name("GLib", env);
        let mut body = vec![
            Chunk::Let {
                name: "error".to_string(),
                is_mut: true,
                value: Box::new(Chunk::NullMutPtr),
                type_: None,
            },
        ];
        let output_vars = trampoline.output_params.iter()
            .filter(|param| param.direction == ParameterDirection::Out && param.name != "error")
            .map(|param| (param, type_mem_mode(env, param)))
            .map(|(param, mode)|
                 Chunk::Let {
                     name: param.name.clone(),
                     is_mut: true,
                     value: Box::new(mode),
                     type_: None,
                 });
        body.extend(output_vars);

        let ret_name = if trampoline.ffi_ret.is_some() { "ret" } else { "_" };

        body.push(
            Chunk::Let {
                name: ret_name.to_string(),
                is_mut: false,
                value: Box::new(Chunk::FfiCall {
                    name: trampoline.finish_func_name.clone(),
                    params: finish_args,
                }),
                type_: None,
            }
        );
        body.push(
            Chunk::Let {
                name: "result".to_string(),
                is_mut: false,
                value: Box::new(Chunk::ErrorResultReturn {
                    value: Box::new(result),
                }),
                type_: None,
            }
        );
        body.push(
            Chunk::Let {
                name: "callback".to_string(),
                is_mut: false,
                value: Box::new(Chunk::Custom("Box::from_raw(user_data as *mut _)".into())),
                type_: Some(Box::new(Chunk::Custom(format!("Box<Box<{}>>", trampoline.bound_name)))),
            }
        );
        body.push(
            Chunk::Call {
                func_name: "callback".to_string(),
                arguments: vec![Chunk::Name("result".to_string())],
            }
        );

        let parameters = vec![
            Param { name: "_source_object".to_string(), typ: format!("*mut {}::GObject", gobject_crate_name) },
            Param { name: "res".to_string(), typ: format!("*mut {}::GAsyncResult", gio_crate_name) },
            Param { name: "user_data".to_string(), typ: format!("{}::gpointer", glib_crate_name) },
        ];

        chunks.push(Chunk::ExternCFunc {
            name: format!("{}<{}: {}>", trampoline.name, trampoline.bound_name, trampoline.callback_type),
            parameters,
            body: Box::new(Chunk::Chunks(body)),
            return_value: None,
            bounds: String::new(),
        });
        let chunk = Chunk::Let {
            name: "callback".to_string(),
            is_mut: false,
            value: Box::new(Chunk::Name(format!("{}::<{}>", trampoline.name, trampoline.bound_name))),
            type_: None,
        };
        chunks.push(chunk);
    }

    fn array_length(&self, param: &library::Parameter) -> Option<&String> {
        self.async_trampoline
            .as_ref()
            .and_then(|trampoline|
                 param.array_length
                     .map(|index| &trampoline.output_params[index as usize].name)
            )
    }

    fn add_assertion(&self, chunks: &mut Vec<Chunk>) {
        match self.assertion {
            SafetyAssertionMode::None => (),
            SafetyAssertionMode::Skip => chunks.insert(0, Chunk::AssertSkipInitialized),
            SafetyAssertionMode::InMainThread => {
                chunks.insert(0, Chunk::AssertInitializedAndInMainThread)
            }
        }
    }
    fn add_into_conversion(&self, chunks: &mut Vec<Chunk>) {
        for trans in &self.transformations {
            if let TransformationType::Into {
                ref name,
            } = trans.transformation_type
            {
                let value = Chunk::Custom(format!("{}.into()", name));
                chunks.push(Chunk::Let {
                    name: name.clone(),
                    is_mut: false,
                    value: Box::new(value),
                    type_: None,
                });
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
                if let In = self.parameters[trans.ind_c] {
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

    fn generate_call(
        &self,
        calls: &HashMap<usize, (usize, Option<String>, Vec<&Trampoline>)>,
    ) -> Chunk {
        let params = self.generate_func_parameters(calls);
        let func = Chunk::FfiCall {
            name: self.glib_name.clone(),
            params,
        };
        func
    }
    fn generate_call_conversion(&self, call: Chunk) -> Chunk {
        Chunk::FfiCallConversion {
            ret: self.ret.ret.clone(),
            array_length_name: self.find_array_length_name(""),
            call: Box::new(call),
        }
    }
    fn generate_func_parameters(
        &self,
        calls: &HashMap<usize, (usize, Option<String>, Vec<&Trampoline>)>,
    ) -> Vec<Chunk> {
        let mut params = Vec::new();
        for trans in &self.transformations {
            if !trans.transformation_type.is_to_glib() {
                continue;
            }
            let par = &self.parameters[trans.ind_c];
            let chunk = match *par {
                In => Chunk::FfiCallParameter {
                    transformation_type: trans.transformation_type.clone(),
                },
                Out { ref parameter, .. } => Chunk::FfiCallOutParameter {
                    par: parameter.clone(),
                },
            };
            params.push(chunk);
        }
        for (user_data_index, (pos, _, callbacks)) in calls.iter() {
            let all_call = callbacks.iter().all(|c| c.is_call);
            params.insert(*user_data_index as _, Chunk::FfiCallParameter {
                    transformation_type: TransformationType::ToGlibDirect {
                        name: if all_call {
                            format!("super_callback{} as *const _ as usize as *mut _", pos)
                        } else {
                            format!("Box::into_raw(super_callback{}) as *mut _", pos)
                        },
                    }});
        }
        for destroy in self.destroys.iter() {
            params.insert(destroy.destroy_index,
                          Chunk::FfiCallParameter {
                              transformation_type: TransformationType::ToGlibDirect {
                                  name: format!("destroy_call{}", destroy.destroy_index),
                              }});
        }
        params
    }
    fn get_outs(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|par| if let Out { .. } = *par {
                true
            } else {
                false
            })
            .collect()
    }
    fn get_outs_without_error(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|par| if let Out { ref parameter, .. } = *par {
                !parameter.is_error
            } else {
                false
            })
            .collect()
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
                        } if array_length_name == &parameter.name => true,
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
}

fn c_type_mem_mode_lib(env: &Env, typ: library::TypeId, caller_allocates: bool,
                       transfer: library::Transfer) -> OutMemMode {
    use self::OutMemMode::*;
    match ConversionType::of(env, typ) {
        ConversionType::Pointer => if caller_allocates {
            UninitializedNamed(rust_type(env, typ).unwrap())
        } else {
            use library::Type::*;
            let type_ = env.library.type_(typ);
            match *type_ {
                Fundamental(fund)
                    if fund == library::Fundamental::Utf8
                        || fund == library::Fundamental::OsString
                        || fund == library::Fundamental::Filename =>
                        {
                            if transfer == library::Transfer::Full {
                                NullMutPtr
                            } else {
                                NullPtr
                            }
                        }
                _ => NullMutPtr,
            }
        },
        _ => Uninitialized,
    }
}

fn c_type_mem_mode(env: &Env, parameter: &AnalysisCParameter) -> OutMemMode {
    c_type_mem_mode_lib(env, parameter.typ, parameter.caller_allocates, parameter.transfer)
}

fn type_mem_mode(env: &Env, parameter: &library::Parameter) -> Chunk {
    match ConversionType::of(env, parameter.typ) {
        ConversionType::Pointer => if parameter.caller_allocates {
            Chunk::UninitializedNamed {
                name: rust_type(env, parameter.typ).unwrap(),
            }
        } else {
            use library::Type::*;
            let type_ = env.library.type_(parameter.typ);
            match *type_ {
                Fundamental(fund)
                    if fund == library::Fundamental::Utf8
                        || fund == library::Fundamental::OsString
                        || fund == library::Fundamental::Filename =>
                        {
                            if parameter.transfer == library::Transfer::Full {
                                Chunk::NullMutPtr
                            } else {
                                Chunk::NullPtr
                            }
                        }
                _ => Chunk::NullMutPtr,
            }
        },
        _ => Chunk::Uninitialized,
    }
}

fn crate_name(name: &str, env: &Env) -> String {
    let id = env.library.find_namespace(name).expect("namespace from crate name");
    let namespace = env.library.namespace(id);
    let name = nameutil::crate_name(&namespace.name);
    if id == namespaces::MAIN {
        return "ffi".to_string();
    }
    format!("{}_ffi", name)
}
