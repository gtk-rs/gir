use crate::{
    analysis::{
        self,
        conversion_type::ConversionType,
        function_parameters::{
            CParameter as AnalysisCParameter, Transformation, TransformationType,
        },
        functions::{find_index_to_ignore, AsyncTrampoline},
        out_parameters::Mode,
        return_value,
        rust_type::RustType,
        safety_assertion_mode::SafetyAssertionMode,
        trampoline_parameters,
        trampolines::Trampoline,
    },
    chunk::{parameter_ffi_call_out, Chunk, Param, TupleMode},
    env::Env,
    library::{self, ParameterDirection, TypeId},
    nameutil::{is_gstring, use_gio_type, use_glib_if_needed, use_glib_type},
    traits::*,
};
use std::collections::{hash_map::Entry, BTreeMap, HashMap};

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
use self::Parameter::*;

#[derive(Clone, Debug, Eq, PartialEq)]
enum OutMemMode {
    Uninitialized,
    UninitializedNamed(String),
    NullPtr,
    NullMutPtr,
}

impl OutMemMode {
    fn is_uninitialized(&self) -> bool {
        matches!(*self, OutMemMode::Uninitialized)
    }
}

#[derive(Clone, Default)]
struct ReturnValue {
    pub ret: return_value::Info,
}

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
    in_unsafe: bool,
    outs_mode: Mode,
    assertion: SafetyAssertionMode,
}

// Key: user data index
// Value: (global position used as id, type, callbacks)
type FuncParameters<'a> = BTreeMap<usize, FuncParameter<'a>>;

struct FuncParameter<'a> {
    pos: usize,
    full_type: Option<(String, String)>,
    callbacks: Vec<&'a Trampoline>,
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
            parameter: parameter_ffi_call_out::Parameter::new(
                parameter,
                mem_mode.is_uninitialized(),
            ),
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
    pub fn in_unsafe(&mut self, in_unsafe: bool) -> &mut Builder {
        self.in_unsafe = in_unsafe;
        self
    }
    pub fn generate(&self, env: &Env, bounds: String, bounds_names: String) -> Chunk {
        let mut body = Vec::new();

        let mut uninitialized_vars = if self.outs_as_return {
            self.write_out_variables(&mut body, env)
        } else {
            Vec::new()
        };

        let mut group_by_user_data = FuncParameters::new();

        // We group arguments by callbacks.
        if !self.callbacks.is_empty() || !self.destroys.is_empty() {
            for (pos, callback) in self.callbacks.iter().enumerate() {
                let user_data_index = callback.user_data_index;
                if group_by_user_data.get(&user_data_index).is_some() {
                    continue;
                }
                let calls = self
                    .callbacks
                    .iter()
                    .filter(|c| c.user_data_index == user_data_index)
                    .collect::<Vec<_>>();
                group_by_user_data.insert(
                    user_data_index,
                    FuncParameter {
                        pos,
                        full_type: if calls.len() > 1 {
                            if calls.iter().all(|c| c.scope.is_call()) {
                                Some((
                                    format!(
                                        "&({})",
                                        calls
                                            .iter()
                                            .map(|c| format!("&{}", c.bound_name))
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    ),
                                    format!(
                                        "&mut ({})",
                                        calls
                                            .iter()
                                            .map(|c| format!("&mut {}", c.bound_name))
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    ),
                                ))
                            } else {
                                let s = format!(
                                    "Box_<({})>",
                                    calls
                                        .iter()
                                        .map(|c| format!("&{}", c.bound_name))
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                );
                                Some((s.clone(), s))
                            }
                        } else {
                            None
                        },
                        callbacks: calls,
                    },
                );
            }
        }

        let call = self.generate_call(&group_by_user_data);
        let call = self.generate_call_conversion(call, &mut uninitialized_vars);
        let ret = self.generate_out_return(&mut uninitialized_vars);
        let (call, ret) = self.apply_outs_mode(call, ret, &mut uninitialized_vars);

        body.push(call);
        self.write_out_uninitialized(&mut body, uninitialized_vars);
        if let Some(chunk) = ret {
            body.push(chunk);
        }

        let mut chunks = Vec::new();

        self.add_in_into_conversions(&mut chunks);
        self.add_in_array_lengths(&mut chunks);
        self.add_assertion(&mut chunks);

        if !self.callbacks.is_empty() || !self.destroys.is_empty() {
            // Key: user data index
            // Value: the current pos in the tuple for the given argument.
            let mut poses = HashMap::with_capacity(group_by_user_data.len());
            for trampoline in self.callbacks.iter() {
                *poses
                    .entry(&trampoline.user_data_index)
                    .or_insert_with(|| 0) += 1;
            }
            let mut poses = poses
                .into_iter()
                .filter(|(_, x)| *x > 1)
                .map(|(x, _)| (x, 0))
                .collect::<HashMap<_, _>>();
            for trampoline in self.callbacks.iter() {
                let user_data_index = trampoline.user_data_index;
                let pos = poses.entry(&trampoline.user_data_index);
                self.add_trampoline(
                    env,
                    &mut chunks,
                    trampoline,
                    &group_by_user_data[&user_data_index].full_type,
                    match pos {
                        Entry::Occupied(ref x) => Some(*x.get()),
                        _ => None,
                    },
                    &bounds,
                    &bounds_names,
                    false,
                );
                pos.and_modify(|x| {
                    *x += 1;
                });
            }
            for destroy in self.destroys.iter() {
                self.add_trampoline(
                    env,
                    &mut chunks,
                    destroy,
                    &group_by_user_data[&destroy.user_data_index].full_type,
                    None, // doesn't matter for destroy
                    &bounds,
                    &bounds_names,
                    true,
                );
            }
            for FuncParameter {
                pos,
                full_type,
                callbacks: calls,
            } in group_by_user_data.values()
            {
                if calls.len() > 1 {
                    chunks.push(Chunk::Let {
                        name: format!("super_callback{}", pos),
                        is_mut: false,
                        value: Box::new(Chunk::Custom(if poses.is_empty() {
                            format!(
                                "Box_::new(Box_::new(({})))",
                                calls
                                    .iter()
                                    .map(|c| format!("{}_data", c.name))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        } else if calls.iter().all(|c| c.scope.is_call()) {
                            format!(
                                "&({})",
                                calls
                                    .iter()
                                    .map(|c| format!("{}_data", c.name))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        } else {
                            format!(
                                "Box_::new(({}))",
                                calls
                                    .iter()
                                    .map(|c| format!("{}_data", c.name))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        })),
                        type_: Some(Box::new(Chunk::Custom(
                            full_type.clone().map(|x| x.0).unwrap(),
                        ))),
                    });
                } else if !calls.is_empty() {
                    chunks.push(Chunk::Let {
                        name: format!("super_callback{}", pos),
                        is_mut: false,
                        value: Box::new(Chunk::Custom(format!(
                            "{}{}_data",
                            if calls[0].scope.is_call() { "&" } else { "" },
                            calls[0].name
                        ))),
                        type_: Some(Box::new(Chunk::Custom(if calls[0].scope.is_call() {
                            format!("&{}", calls[0].bound_name)
                        } else {
                            format!("Box_<{}>", calls[0].bound_name)
                        }))),
                    });
                }
            }
        } else if let Some(ref trampoline) = self.async_trampoline {
            self.add_async_trampoline(env, &mut chunks, trampoline);
        }

        chunks.push(if self.in_unsafe {
            Chunk::Chunks(body)
        } else {
            Chunk::Unsafe(body)
        });
        Chunk::BlockHalf(chunks)
    }

    fn write_out_uninitialized(
        &self,
        body: &mut Vec<Chunk>,
        uninitialized_vars: Vec<(String, bool)>,
    ) {
        for (uninitialized_var, need_from_glib) in uninitialized_vars {
            body.push(Chunk::Let {
                name: uninitialized_var.clone(),
                is_mut: false,
                value: Box::new(Chunk::Custom(format!(
                    "{}{}.assume_init(){}",
                    if need_from_glib { "from_glib(" } else { "" },
                    uninitialized_var,
                    if need_from_glib { ")" } else { "" },
                ))),
                type_: None,
            });
        }
    }

    fn remove_extra_assume_init(
        &self,
        array_length_name: &Option<String>,
        uninitialized_vars: &mut Vec<(String, bool)>,
    ) {
        // To prevent to call twice `.assume_init()` on the length variable, we need to
        // remove them from the `uninitialized_vars` array.
        if let Some(array_length_name) = array_length_name {
            uninitialized_vars.retain(|(x, _)| x != array_length_name);
        }
    }

    fn add_trampoline(
        &self,
        env: &Env,
        chunks: &mut Vec<Chunk>,
        trampoline: &Trampoline,
        full_type: &Option<(String, String)>,
        pos: Option<usize>,
        bounds: &str,
        bounds_names: &str,
        is_destroy: bool,
    ) {
        if !is_destroy {
            if full_type.is_none() {
                if trampoline.scope.is_call() {
                    chunks.push(Chunk::Custom(format!(
                        "let {0}_data: {1} = {0};",
                        trampoline.name, trampoline.bound_name
                    )));
                } else {
                    chunks.push(Chunk::Custom(format!(
                        "let {0}_data: Box_<{1}> = Box_::new({0});",
                        trampoline.name, trampoline.bound_name
                    )));
                }
            } else if trampoline.scope.is_call() {
                chunks.push(Chunk::Custom(format!(
                    "let {0}_data: &{1} = &{0};",
                    trampoline.name, trampoline.bound_name
                )));
            } else {
                chunks.push(Chunk::Custom(format!(
                    "let {0}_data: {1} = {0};",
                    trampoline.name, trampoline.bound_name
                )));
            }
        }

        let mut body = Vec::new();
        let mut arguments = Vec::new();

        for par in trampoline.parameters.transformations.iter() {
            if par.name == "this"
                || trampoline.parameters.c_parameters[par.ind_c].is_real_gpointer(env)
            {
                continue;
            }
            let ty_name = match RustType::try_new(env, par.typ) {
                Ok(x) => x.into_string(),
                _ => String::new(),
            };
            let nullable = trampoline.parameters.rust_parameters[par.ind_rust].nullable;
            let is_fundamental =
                add_chunk_for_type(env, par.typ, par, &mut body, &ty_name, nullable);
            if is_gstring(&ty_name) {
                if *nullable {
                    arguments.push(Chunk::Name(format!("{}.as_ref().as_deref()", par.name)));
                } else {
                    arguments.push(Chunk::Name(format!("{}.as_str()", par.name)));
                }
                continue;
            }
            if *nullable && !is_fundamental {
                arguments.push(Chunk::Name(format!("{}.as_ref().as_ref()", par.name)));
                continue;
            }
            arguments.push(Chunk::Name(format!(
                "{}{}",
                if is_fundamental { "" } else { "&" },
                par.name
            )));
        }

        let func = trampoline
            .parameters
            .c_parameters
            .last()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Unknown".to_owned());

        let mut extra_before_call = "";
        if let Some(full_type) = full_type {
            if is_destroy || trampoline.scope.is_async() {
                body.push(Chunk::Let {
                    name: format!("{}callback", if is_destroy { "_" } else { "" }),
                    is_mut: false,
                    value: Box::new(Chunk::Custom(format!("Box_::from_raw({} as *mut _)", func))),
                    type_: Some(Box::new(Chunk::Custom(full_type.1.clone()))),
                });
            } else {
                body.push(Chunk::Let {
                    name: "callback".to_owned(),
                    is_mut: false,
                    value: Box::new(Chunk::Custom(format!(
                        "{}*({} as *mut _)",
                        if !trampoline.scope.is_call() {
                            "&"
                        } else if pos.is_some() {
                            "&mut "
                        } else {
                            ""
                        },
                        func
                    ))),
                    type_: Some(Box::new(Chunk::Custom(
                        if !trampoline.scope.is_async() && !trampoline.scope.is_call() {
                            format!("&{}", full_type.1)
                        } else {
                            full_type.1.clone()
                        },
                    ))),
                });
                if trampoline.scope.is_async() {
                    body.push(Chunk::Custom(format!(
                        "let callback = callback{}{};",
                        if let Some(pos) = pos {
                            format!(".{}", pos)
                        } else {
                            String::new()
                        },
                        if *trampoline.nullable {
                            ".expect(\"cannot get closure...\")"
                        } else {
                            ""
                        }
                    )));
                    if trampoline.ret.c_type != "void" {
                        extra_before_call = "let res = ";
                    }
                } else if !trampoline.scope.is_call() {
                    if *trampoline.nullable {
                        body.push(Chunk::Custom(format!(
                            "{}if let Some(ref callback) = callback{} {{",
                            if trampoline.ret.c_type != "void" {
                                "let res = "
                            } else {
                                ""
                            },
                            if let Some(pos) = pos {
                                format!(".{}", pos)
                            } else {
                                String::new()
                            }
                        )));
                    } else {
                        body.push(Chunk::Custom(format!(
                            "let callback = callback{}",
                            if let Some(pos) = pos {
                                format!(".{}", pos)
                            } else {
                                String::new()
                            }
                        )));
                        if trampoline.ret.c_type != "void" {
                            body.push(Chunk::Custom("let res = ".to_owned()));
                        }
                    }
                } else {
                    let add = if trampoline.ret.c_type != "void" {
                        "let res = "
                    } else {
                        ""
                    };
                    if !trampoline.scope.is_async() && *trampoline.nullable {
                        body.push(Chunk::Custom(format!(
                            "{}if let Some(ref {}callback) = {} {{",
                            add,
                            if trampoline.scope.is_call() {
                                "mut "
                            } else {
                                ""
                            },
                            if let Some(pos) = pos {
                                format!("(*callback).{}", pos)
                            } else {
                                "*callback".to_owned()
                            }
                        )));
                    } else {
                        body.push(Chunk::Custom(add.to_owned()));
                    }
                }
            }
        } else {
            body.push(Chunk::Let {
                name: format!("{}callback", if is_destroy { "_" } else { "" }),
                is_mut: false,
                value: Box::new(Chunk::Custom(
                    if is_destroy || trampoline.scope.is_async() {
                        format!("Box_::from_raw({} as *mut _)", func)
                    } else if trampoline.scope.is_call() {
                        format!(
                            "{} as *const _ as usize as *mut {}",
                            func, trampoline.bound_name
                        )
                    } else {
                        format!("&*({} as *mut _)", func)
                    },
                )),
                type_: Some(Box::new(Chunk::Custom(
                    if is_destroy || trampoline.scope.is_async() {
                        format!("Box_<{}>", trampoline.bound_name)
                    } else if trampoline.scope.is_call() {
                        format!("*mut {}", trampoline.bound_name)
                    } else {
                        format!("&{}", trampoline.bound_name)
                    },
                ))),
            });
            if !is_destroy && *trampoline.nullable {
                if trampoline.scope.is_async() {
                    body.push(Chunk::Custom(
                        "let callback = (*callback).expect(\"cannot get closure...\");".to_owned(),
                    ));
                    if trampoline.ret.c_type != "void" {
                        extra_before_call = "let res = ";
                    }
                } else {
                    body.push(Chunk::Custom(format!(
                        "{}if let Some(ref {}callback) = {} {{",
                        if trampoline.ret.c_type != "void" {
                            "let res = "
                        } else {
                            ""
                        },
                        if trampoline.scope.is_call() {
                            "mut "
                        } else {
                            ""
                        },
                        if let Some(pos) = pos {
                            format!("(*callback).{}", pos)
                        } else {
                            "*callback".to_owned()
                        }
                    )));
                }
            } else if !is_destroy && trampoline.ret.c_type != "void" {
                extra_before_call = "let res = ";
            }
        }
        if !is_destroy {
            use crate::writer::to_code::ToCode;
            body.push(Chunk::Custom(format!(
                "{}{}({}){}",
                extra_before_call,
                if !*trampoline.nullable {
                    "(*callback)"
                } else if trampoline.scope.is_async() {
                    "callback"
                } else {
                    "\tcallback"
                },
                arguments
                    .iter()
                    .flat_map(|arg| arg.to_code(env))
                    .collect::<Vec<_>>()
                    .join(", "),
                if !extra_before_call.is_empty() || !*trampoline.nullable {
                    ";"
                } else {
                    ""
                }
            )));
            if !trampoline.scope.is_async() && *trampoline.nullable {
                body.push(Chunk::Custom("} else {".to_owned()));
                body.push(Chunk::Custom(
                    "\tpanic!(\"cannot get closure...\")".to_owned(),
                ));
                body.push(Chunk::Custom("};".to_owned()));
            }
            if trampoline.ret.c_type != "void" {
                use crate::codegen::trampoline_to_glib::TrampolineToGlib;

                body.push(Chunk::Custom(format!(
                    "res{}",
                    trampoline.ret.trampoline_to_glib(env)
                )));
            }
        }

        let extern_func = Chunk::ExternCFunc {
            name: format!("{}_func", trampoline.name),
            parameters: trampoline
                .parameters
                .c_parameters
                .iter()
                .skip(1) // to skip the generated this
                .map(|p| {
                    if p.is_real_gpointer(env) {
                        Param {
                            name: p.name.clone(),
                            typ: use_glib_if_needed(env, "ffi::gpointer"),
                        }
                    } else {
                        Param {
                            name: p.name.clone(),
                            typ: crate::analysis::ffi_type::ffi_type(env, p.typ, &p.c_type)
                                .expect("failed to write c_type")
                                .into_string(),
                        }
                    }
                })
                .collect::<Vec<_>>(),
            body: Box::new(Chunk::Chunks(body)),
            return_value: if trampoline.ret.c_type != "void" {
                let p = &trampoline.ret;
                Some(
                    crate::analysis::ffi_type::ffi_type(env, p.typ, &p.c_type)
                        .expect("failed to write c_type")
                        .into_string(),
                )
            } else {
                None
            },
            bounds: bounds.to_owned(),
        };

        chunks.push(extern_func);
        let bounds_str = if bounds_names.is_empty() {
            String::new()
        } else {
            format!("::<{}>", bounds_names)
        };
        if !is_destroy {
            if *trampoline.nullable {
                chunks.push(Chunk::Custom(format!(
                    "let {0} = if {0}_data.is_some() {{ Some({0}_func{1} as _) }} else {{ None }};",
                    trampoline.name, bounds_str
                )));
            } else {
                chunks.push(Chunk::Custom(format!(
                    "let {0} = Some({0}_func{1} as _);",
                    trampoline.name, bounds_str
                )));
            }
        } else {
            chunks.push(Chunk::Custom(format!(
                "let destroy_call{} = Some({}_func{} as _);",
                trampoline.destroy_index, trampoline.name, bounds_str
            )));
        }
    }

    fn add_async_trampoline(
        &self,
        env: &Env,
        chunks: &mut Vec<Chunk>,
        trampoline: &AsyncTrampoline,
    ) {
        chunks.push(Chunk::Let {
            name: "user_data".to_string(),
            is_mut: false,
            value: Box::new(Chunk::Custom("Box_::new(callback)".into())),
            type_: Some(Box::new(Chunk::Custom(format!(
                "Box_<{}>",
                trampoline.bound_name
            )))),
        });

        let mut finish_args = vec![];
        let mut uninitialized_vars = Vec::new();
        if trampoline.is_method {
            finish_args.push(Chunk::Cast {
                name: "_source_object".to_string(),
                type_: "*mut _".to_string(),
            });
        }
        let mut found_async_result = false;
        finish_args.extend(
            trampoline
                .output_params
                .iter()
                .filter(|out| {
                    out.lib_par.direction == ParameterDirection::Out
                        || out.lib_par.typ.full_name(&env.library) == "Gio.AsyncResult"
                })
                .map(|out| {
                    if out.lib_par.typ.full_name(&env.library) == "Gio.AsyncResult" {
                        found_async_result = true;
                        return Chunk::Name("res".to_string());
                    }
                    let kind = type_mem_mode(env, &out.lib_par);
                    let mut par: parameter_ffi_call_out::Parameter = out.into();
                    if kind.is_uninitialized() {
                        par.is_uninitialized = true;
                        uninitialized_vars.push((
                            out.lib_par.name.clone(),
                            self.check_if_need_glib_conversion(env, out.lib_par.typ),
                        ));
                    }
                    Chunk::FfiCallOutParameter { par }
                }),
        );
        assert!(
            found_async_result,
            "The check *wasn't* performed in analysis part: Guillaume was wrong!"
        );
        let index_to_ignore = find_index_to_ignore(
            trampoline.output_params.iter().map(|par| &par.lib_par),
            trampoline.ffi_ret.as_ref().map(|ret| &ret.lib_par),
        );
        let mut result: Vec<_> = trampoline
            .output_params
            .iter()
            .enumerate()
            .filter(|&(index, out)| {
                out.lib_par.direction == ParameterDirection::Out
                    && out.lib_par.name != "error"
                    && Some(index) != index_to_ignore
            })
            .map(|(_, out)| {
                let value = Chunk::Custom(out.lib_par.name.clone());
                let mem_mode = c_type_mem_mode_lib(
                    env,
                    out.lib_par.typ,
                    out.lib_par.caller_allocates,
                    out.lib_par.transfer,
                );
                if let OutMemMode::UninitializedNamed(_) = mem_mode {
                    value
                } else {
                    let array_length_name = self.array_length(&out).cloned();
                    self.remove_extra_assume_init(&array_length_name, &mut uninitialized_vars);
                    Chunk::FromGlibConversion {
                        mode: out.into(),
                        array_length_name,
                        value: Box::new(value),
                    }
                }
            })
            .collect();

        if let Some(ref ffi_ret) = trampoline.ffi_ret {
            let mem_mode = c_type_mem_mode_lib(
                env,
                ffi_ret.lib_par.typ,
                ffi_ret.lib_par.caller_allocates,
                ffi_ret.lib_par.transfer,
            );
            let value = Chunk::Name("ret".to_string());
            if let OutMemMode::UninitializedNamed(_) = mem_mode {
                result.insert(0, value);
            } else {
                let array_length_name = self.array_length(ffi_ret).cloned();
                self.remove_extra_assume_init(&array_length_name, &mut uninitialized_vars);
                result.insert(
                    0,
                    Chunk::FromGlibConversion {
                        mode: ffi_ret.into(),
                        array_length_name,
                        value: Box::new(value),
                    },
                );
            }
        }

        let result = Chunk::Tuple(result, TupleMode::WithUnit);
        let mut body = vec![Chunk::Let {
            name: "error".to_string(),
            is_mut: true,
            value: Box::new(Chunk::NullMutPtr),
            type_: None,
        }];
        let output_vars = trampoline
            .output_params
            .iter()
            .filter(|out| {
                out.lib_par.direction == ParameterDirection::Out && out.lib_par.name != "error"
            })
            .map(|out| Chunk::Let {
                name: out.lib_par.name.clone(),
                is_mut: true,
                value: Box::new(type_mem_mode(env, &out.lib_par)),
                type_: None,
            });
        body.extend(output_vars);

        let ret_name = if trampoline.ffi_ret.is_some() {
            "ret"
        } else {
            "_"
        };

        body.push(Chunk::Let {
            name: ret_name.to_string(),
            is_mut: false,
            value: Box::new(Chunk::FfiCall {
                name: trampoline.finish_func_name.clone(),
                params: finish_args,
            }),
            type_: None,
        });
        self.write_out_uninitialized(&mut body, uninitialized_vars);
        body.push(Chunk::Let {
            name: "result".to_string(),
            is_mut: false,
            value: Box::new(Chunk::ErrorResultReturn {
                value: Box::new(result),
            }),
            type_: None,
        });
        body.push(Chunk::Let {
            name: "callback".to_string(),
            is_mut: false,
            value: Box::new(Chunk::Custom("Box_::from_raw(user_data as *mut _)".into())),
            type_: Some(Box::new(Chunk::Custom(format!(
                "Box_<{}>",
                trampoline.bound_name
            )))),
        });
        body.push(Chunk::Call {
            func_name: "callback".to_string(),
            arguments: vec![Chunk::Name("result".to_string())],
        });

        let parameters = vec![
            Param {
                name: "_source_object".to_string(),
                typ: format!("*mut {}", use_glib_type(env, "gobject_ffi::GObject")),
            },
            Param {
                name: "res".to_string(),
                typ: format!("*mut {}", use_gio_type(env, "ffi::GAsyncResult")),
            },
            Param {
                name: "user_data".to_string(),
                typ: use_glib_if_needed(env, "ffi::gpointer"),
            },
        ];

        chunks.push(Chunk::ExternCFunc {
            name: format!(
                "{}<{}: {}>",
                trampoline.name, trampoline.bound_name, trampoline.callback_type
            ),
            parameters,
            body: Box::new(Chunk::Chunks(body)),
            return_value: None,
            bounds: String::new(),
        });
        let chunk = Chunk::Let {
            name: "callback".to_string(),
            is_mut: false,
            value: Box::new(Chunk::Name(format!(
                "{}::<{}>",
                trampoline.name, trampoline.bound_name
            ))),
            type_: None,
        };
        chunks.push(chunk);
    }

    fn array_length(&self, param: &analysis::Parameter) -> Option<&String> {
        self.async_trampoline.as_ref().and_then(|trampoline| {
            param
                .lib_par
                .array_length
                .map(|index| &trampoline.output_params[index as usize].lib_par.name)
        })
    }

    fn add_assertion(&self, chunks: &mut Vec<Chunk>) {
        match self.assertion {
            SafetyAssertionMode::None => (),
            x => chunks.insert(0, Chunk::AssertInit(x)),
        }
    }

    fn add_in_into_conversions(&self, chunks: &mut Vec<Chunk>) {
        for trans in &self.transformations {
            if let TransformationType::Into {
                ref name,
                ref typ,
                ref nullable,
            } = &trans.transformation_type
            {
                if let In = self.parameters[trans.ind_c] {
                    let (value, typ) = if *nullable {
                        (
                            Chunk::Custom(format!("{}.map(|p| p.into())", name)),
                            Chunk::Custom(format!("Option<{}>", typ)),
                        )
                    } else {
                        (
                            Chunk::Custom(format!("{}.into()", name)),
                            Chunk::Custom(format!("{}", typ)),
                        )
                    };
                    chunks.push(Chunk::Let {
                        name: name.clone(),
                        is_mut: false,
                        value: Box::new(value),
                        type_: Some(Box::new(typ)),
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

    fn generate_call(&self, calls: &FuncParameters<'_>) -> Chunk {
        let params = self.generate_func_parameters(calls);
        let func = Chunk::FfiCall {
            name: self.glib_name.clone(),
            params,
        };
        func
    }
    fn generate_call_conversion(
        &self,
        call: Chunk,
        uninitialized_vars: &mut Vec<(String, bool)>,
    ) -> Chunk {
        let array_length_name = self.find_array_length_name("");
        self.remove_extra_assume_init(&array_length_name, uninitialized_vars);
        Chunk::FfiCallConversion {
            ret: self.ret.ret.clone(),
            array_length_name,
            call: Box::new(call),
        }
    }
    fn generate_func_parameters(&self, calls: &FuncParameters<'_>) -> Vec<Chunk> {
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
                Out { parameter, .. } => Chunk::FfiCallOutParameter {
                    par: parameter.clone(),
                },
            };
            params.push(chunk);
        }
        let mut to_insert = Vec::new();
        for (user_data_index, FuncParameter { pos, callbacks, .. }) in calls.iter() {
            let all_call = callbacks.iter().all(|c| c.scope.is_call());
            to_insert.push((
                *user_data_index,
                Chunk::FfiCallParameter {
                    transformation_type: TransformationType::ToGlibDirect {
                        name: if all_call {
                            format!("super_callback{} as *const _ as usize as *mut _", pos)
                        } else {
                            format!("Box_::into_raw(super_callback{}) as *mut _", pos)
                        },
                    },
                },
            ));
        }
        for destroy in self.destroys.iter() {
            to_insert.push((
                destroy.destroy_index,
                Chunk::FfiCallParameter {
                    transformation_type: TransformationType::ToGlibDirect {
                        name: format!("destroy_call{}", destroy.destroy_index),
                    },
                },
            ));
        }
        to_insert.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
        for (pos, data) in to_insert {
            params.insert(pos, data)
        }
        params
    }
    fn get_outs(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|par| matches!(*par, Out { .. }))
            .collect()
    }
    fn get_outs_without_error(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|par| {
                if let Out { parameter, .. } = par {
                    !parameter.is_error
                } else {
                    false
                }
            })
            .collect()
    }
    fn check_if_need_glib_conversion(&self, env: &Env, typ: TypeId) -> bool {
        // TODO: maybe improve this part to potentially handle more cases than just glib::Pid?
        matches!(
            env.type_(typ),
            library::Type::Alias(a) if a.c_identifier == "GPid"
        )
    }
    fn write_out_variables(&self, v: &mut Vec<Chunk>, env: &Env) -> Vec<(String, bool)> {
        let mut ret = Vec::new();
        let outs = self.get_outs();

        for par in outs {
            if let Out {
                parameter,
                mem_mode,
            } = par
            {
                let val = self.get_uninitialized(mem_mode);
                if val.is_uninitialized() {
                    ret.push((
                        parameter.name.clone(),
                        self.check_if_need_glib_conversion(env, parameter.typ),
                    ));
                }
                let chunk = Chunk::Let {
                    name: parameter.name.clone(),
                    is_mut: true,
                    value: Box::new(val),
                    type_: None,
                };
                v.push(chunk);
            }
        }
        ret
    }
    fn get_uninitialized(&self, mem_mode: &OutMemMode) -> Chunk {
        use self::OutMemMode::*;
        match mem_mode {
            Uninitialized => Chunk::Uninitialized,
            UninitializedNamed(ref name) => Chunk::UninitializedNamed { name: name.clone() },
            NullPtr => Chunk::NullPtr,
            NullMutPtr => Chunk::NullMutPtr,
        }
    }

    #[allow(clippy::blocks_in_if_conditions)]
    fn generate_out_return(&self, uninitialized_vars: &mut Vec<(String, bool)>) -> Option<Chunk> {
        if !self.outs_as_return {
            return None;
        }
        let outs = self.get_outs_without_error();
        let mut chs: Vec<Chunk> = Vec::with_capacity(outs.len());
        for par in outs {
            if let Out {
                parameter,
                mem_mode,
            } = par
            {
                if self.transformations.iter().any(|tr| {
                    matches!(
                        &tr.transformation_type,
                        TransformationType::Length {
                             array_length_name,
                            ..
                        } if array_length_name == &parameter.name
                    )
                }) {
                    continue;
                }

                chs.push(self.out_parameter_to_return(parameter, mem_mode, uninitialized_vars));
            }
        }
        let chunk = Chunk::Tuple(chs, TupleMode::Auto);
        Some(chunk)
    }
    fn out_parameter_to_return(
        &self,
        parameter: &parameter_ffi_call_out::Parameter,
        mem_mode: &OutMemMode,
        uninitialized_vars: &mut Vec<(String, bool)>,
    ) -> Chunk {
        let value = Chunk::Custom(parameter.name.clone());
        if let OutMemMode::UninitializedNamed(_) = mem_mode {
            value
        } else {
            let array_length_name = self.find_array_length_name(&parameter.name);
            self.remove_extra_assume_init(&array_length_name, uninitialized_vars);
            Chunk::FromGlibConversion {
                mode: parameter.into(),
                array_length_name,
                value: Box::new(value),
            }
        }
    }
    fn apply_outs_mode(
        &self,
        call: Chunk,
        ret: Option<Chunk>,
        uninitialized_vars: &mut Vec<(String, bool)>,
    ) -> (Chunk, Option<Chunk>) {
        use crate::analysis::out_parameters::Mode::*;
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
                self.remove_extra_assume_init(&array_length_name, uninitialized_vars);
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
        self.transformations.iter().find_map(|tr| {
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
    }
}

fn c_type_mem_mode_lib(
    env: &Env,
    typ: library::TypeId,
    caller_allocates: bool,
    transfer: library::Transfer,
) -> OutMemMode {
    use self::OutMemMode::*;
    match ConversionType::of(env, typ) {
        ConversionType::Pointer => {
            if caller_allocates {
                UninitializedNamed(RustType::try_new(env, typ).unwrap().into_string())
            } else {
                use crate::library::Type::*;
                let type_ = env.library.type_(typ);
                match type_ {
                    Fundamental(library::Fundamental::Utf8)
                    | Fundamental(library::Fundamental::OsString)
                    | Fundamental(library::Fundamental::Filename) => {
                        if transfer == library::Transfer::Full {
                            NullMutPtr
                        } else {
                            NullPtr
                        }
                    }
                    _ => NullMutPtr,
                }
            }
        }
        _ => Uninitialized,
    }
}

fn c_type_mem_mode(env: &Env, parameter: &AnalysisCParameter) -> OutMemMode {
    c_type_mem_mode_lib(
        env,
        parameter.typ,
        parameter.caller_allocates,
        parameter.transfer,
    )
}

fn type_mem_mode(env: &Env, parameter: &library::Parameter) -> Chunk {
    match ConversionType::of(env, parameter.typ) {
        ConversionType::Pointer => {
            if parameter.caller_allocates {
                Chunk::UninitializedNamed {
                    name: RustType::try_new(env, parameter.typ).unwrap().into_string(),
                }
            } else {
                use crate::library::Type::*;
                let type_ = env.library.type_(parameter.typ);
                match type_ {
                    Fundamental(library::Fundamental::Utf8)
                    | Fundamental(library::Fundamental::OsString)
                    | Fundamental(library::Fundamental::Filename) => {
                        if parameter.transfer == library::Transfer::Full {
                            Chunk::NullMutPtr
                        } else {
                            Chunk::NullPtr
                        }
                    }
                    _ => Chunk::NullMutPtr,
                }
            }
        }
        _ => Chunk::Uninitialized,
    }
}

fn add_chunk_for_type(
    env: &Env,
    typ_: library::TypeId,
    par: &trampoline_parameters::Transformation,
    body: &mut Vec<Chunk>,
    ty_name: &str,
    nullable: library::Nullable,
) -> bool {
    let type_ = env.type_(typ_);
    match type_ {
        library::Type::Fundamental(x) if !x.requires_conversion() => true,
        library::Type::Fundamental(library::Fundamental::Boolean) => {
            body.push(Chunk::Custom(format!(
                "let {0} = from_glib({0});",
                par.name
            )));
            true
        }
        library::Type::Fundamental(library::Fundamental::UniChar) => {
            body.push(Chunk::Custom(format!(
                "let {0} = std::convert::TryFrom::try_from({0})\
                     .expect(\"conversion from an invalid Unicode value attempted\");",
                par.name
            )));
            true
        }
        library::Type::Alias(_) if ty_name == "glib::Pid" => {
            body.push(Chunk::Custom(format!(
                "let {0} = from_glib({0});",
                par.name
            )));
            true
        }
        library::Type::Alias(x) => add_chunk_for_type(env, x.typ, par, body, ty_name, nullable),
        x => {
            let (begin, end) =
                crate::codegen::trampoline_from_glib::from_glib_xxx(par.transfer, true);

            let type_name;
            if is_gstring(ty_name) {
                if *nullable {
                    if par.conversion_type == ConversionType::Borrow {
                        type_name = String::from(": Borrowed<Option<glib::GString>>");
                    } else {
                        type_name = String::from(": Option<glib::GString>");
                    }
                } else if par.conversion_type == ConversionType::Borrow {
                    type_name = String::from(": Borrowed<glib::GString>");
                } else {
                    type_name = String::from(": GString");
                }
            } else if par.transfer == library::Transfer::None && *nullable {
                if par.conversion_type == ConversionType::Borrow {
                    type_name = format!(": Borrowed<Option<{}>>", ty_name);
                } else {
                    type_name = format!(": Option<{}>", ty_name);
                }
            } else {
                type_name = String::from("");
            }

            body.push(Chunk::Custom(format!(
                "let {1}{3} = {0}{1}{2};",
                begin, par.name, end, type_name
            )));
            x.is_fundamental()
        }
    }
}
