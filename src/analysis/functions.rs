/*
 * TODO: better heuristic (https://bugzilla.gnome.org/show_bug.cgi?id=623635#c5)
 * TODO: ProgressCallback types (not specific to async).
 * TODO: add annotation for methods like g_file_replace_contents_bytes_async where the finish
 * method has a different prefix.
 */

use crate::{
    analysis::{
        self,
        bounds::{Bounds, CallbackInfo},
        function_parameters::{self, CParameter, Parameters, Transformation, TransformationType},
        imports::Imports,
        is_gpointer,
        out_parameters::{self, use_function_return_for_result},
        ref_mode::RefMode,
        return_value,
        rust_type::*,
        safety_assertion_mode::SafetyAssertionMode,
        signatures::{Signature, Signatures},
        trampolines::Trampoline,
    },
    config::{self, gobjects::GStatus},
    env::Env,
    library::{self, Function, FunctionKind, ParameterDirection, ParameterScope, Transfer, Type},
    nameutil,
    traits::*,
    version::Version,
};
use log::warn;
use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
};

use super::special_functions;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Comment,
    Private,
    Hidden,
}

impl Visibility {
    pub fn hidden(self) -> bool {
        self == Self::Hidden
    }

    pub fn code_visible(self) -> bool {
        matches!(self, Self::Private | Self::Public)
    }
}

#[derive(Clone, Debug)]
pub struct AsyncTrampoline {
    pub is_method: bool,
    pub name: String,
    pub finish_func_name: String,
    pub callback_type: String,
    pub bound_name: char,
    pub output_params: Vec<analysis::Parameter>,
    pub ffi_ret: Option<analysis::Parameter>,
}

#[derive(Clone, Debug)]
pub struct AsyncFuture {
    pub is_method: bool,
    pub name: String,
    pub success_parameters: String,
    pub error_parameters: String,
    pub assertion: SafetyAssertionMode,
}

#[derive(Debug)]
pub struct Info {
    pub name: String,
    pub func_name: String,
    pub new_name: Option<String>,
    pub glib_name: String,
    pub status: GStatus,
    pub kind: library::FunctionKind,
    pub visibility: Visibility,
    pub type_name: Result,
    pub parameters: Parameters,
    pub ret: return_value::Info,
    pub bounds: Bounds,
    pub outs: out_parameters::Info,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub not_version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub assertion: SafetyAssertionMode,
    pub doc_hidden: bool,
    pub doc_ignore_parameters: HashSet<String>,
    pub r#async: bool,
    pub unsafe_: bool,
    pub trampoline: Option<AsyncTrampoline>,
    pub callbacks: Vec<Trampoline>,
    pub destroys: Vec<Trampoline>,
    pub remove_params: Vec<usize>,
    pub async_future: Option<AsyncFuture>,
}

impl Info {
    pub fn codegen_name(&self) -> &str {
        self.new_name.as_ref().unwrap_or(&self.name)
    }

    pub fn is_special(&self) -> bool {
        self.codegen_name()
            .trim_end_matches('_')
            .rsplit('_')
            .next()
            .map_or(false, |i| i.parse::<special_functions::Type>().is_ok())
    }

    // returns whether the method can be linked in the docs
    pub fn should_be_doc_linked(&self, env: &Env) -> bool {
        !self.status.ignored()
            && (self.status.manual() || self.visibility.code_visible())
            && !self.is_special()
            && !self.is_async_finish(env)
    }

    pub fn doc_link(
        &self,
        parent: Option<&str>,
        visible_parent: Option<&str>,
        is_self: bool,
    ) -> String {
        if let Some(p) = parent {
            if is_self {
                format!("[`{f}()`][Self::{f}()]", f = self.codegen_name())
            } else {
                format!(
                    "[`{visible_parent}::{f}()`][crate::{p}::{f}()]",
                    visible_parent = visible_parent.unwrap_or(p),
                    p = p,
                    f = self.codegen_name()
                )
            }
        } else {
            format!(
                "[`{fn_name}()`][crate::{fn_name}()]",
                fn_name = self.codegen_name()
            )
        }
    }

    pub fn is_async_finish(&self, env: &Env) -> bool {
        let has_async_result = self
            .parameters
            .rust_parameters
            .iter()
            .any(|param| param.typ.full_name(&env.library) == "Gio.AsyncResult");
        self.name.ends_with("_finish") && has_async_result
    }
}

pub fn analyze<F: Borrow<library::Function>>(
    env: &Env,
    functions: &[F],
    type_tid: library::TypeId,
    in_trait: bool,
    is_boxed: bool,
    obj: &config::gobjects::GObject,
    imports: &mut Imports,
    mut signatures: Option<&mut Signatures>,
    deps: Option<&[library::TypeId]>,
) -> Vec<Info> {
    let mut funcs = Vec::new();

    'func: for func in functions {
        let func = func.borrow();

        let configured_functions = obj.functions.matched(&func.name);
        let mut status = obj.status;
        for f in configured_functions.iter() {
            match f.status {
                GStatus::Ignore => continue 'func,
                GStatus::Manual => {
                    status = GStatus::Manual;
                    break;
                }
                GStatus::Generate => (),
            }
        }

        if env.is_totally_deprecated(func.deprecated_version) {
            continue;
        }
        let name = nameutil::mangle_keywords(&*func.name).into_owned();
        let signature_params = Signature::new(func);
        let mut not_version = None;
        if func.kind == library::FunctionKind::Method {
            if let Some(deps) = deps {
                let (has, version) = signature_params.has_in_deps(env, &name, deps);
                if has {
                    match version {
                        Some(v) if v > env.config.min_cfg_version => not_version = version,
                        _ => continue,
                    }
                }
            }
        }
        if let Some(signatures) = signatures.as_mut() {
            signatures.insert(name.clone(), signature_params);
        }

        let mut info = analyze_function(
            env,
            obj,
            &func.name,
            name,
            status,
            func,
            type_tid,
            in_trait,
            is_boxed,
            &configured_functions,
            imports,
        );
        info.not_version = not_version;
        funcs.push(info);
    }

    funcs
}

fn fixup_gpointer_parameter(
    env: &Env,
    type_tid: library::TypeId,
    is_boxed: bool,
    parameters: &mut Parameters,
    idx: usize,
) {
    use crate::analysis::ffi_type;

    let instance_parameter = idx == 0;

    let glib_name = env.library.type_(type_tid).get_glib_name().unwrap();
    let ffi_name = ffi_type::ffi_type(env, type_tid, &glib_name).unwrap();
    let pointer_type = if is_boxed { "*const" } else { "*mut" };
    parameters.rust_parameters[idx].typ = type_tid;
    parameters.c_parameters[idx].typ = type_tid;
    parameters.c_parameters[idx].instance_parameter = instance_parameter;
    parameters.c_parameters[idx].ref_mode = RefMode::ByRef;
    parameters.c_parameters[idx].transfer = Transfer::None;
    parameters.transformations[idx] = Transformation {
        ind_c: idx,
        ind_rust: Some(idx),
        transformation_type: TransformationType::ToGlibPointer {
            name: parameters.rust_parameters[idx].name.clone(),
            instance_parameter,
            transfer: Transfer::None,
            ref_mode: RefMode::ByRef,
            to_glib_extra: String::new(),
            explicit_target_type: format!("{} {}", pointer_type, ffi_name.as_str()),
            pointer_cast: format!(
                " as {}",
                nameutil::use_glib_if_needed(env, "ffi::gconstpointer")
            ),
            in_trait: false,
            nullable: false,
        },
    };
}

fn fixup_special_functions(
    env: &Env,
    name: &str,
    type_tid: library::TypeId,
    is_boxed: bool,
    parameters: &mut Parameters,
) {
    // Workaround for some _hash() / _compare() / _equal() functions taking
    // "gconstpointer" as arguments instead of the actual type
    if name == "hash"
        && parameters.c_parameters.len() == 1
        && parameters.c_parameters[0].c_type == "gconstpointer"
    {
        fixup_gpointer_parameter(env, type_tid, is_boxed, parameters, 0);
    }

    if (name == "compare" || name == "equal" || name == "is_equal")
        && parameters.c_parameters.len() == 2
        && parameters.c_parameters[0].c_type == "gconstpointer"
        && parameters.c_parameters[1].c_type == "gconstpointer"
    {
        fixup_gpointer_parameter(env, type_tid, is_boxed, parameters, 0);
        fixup_gpointer_parameter(env, type_tid, is_boxed, parameters, 1);
    }
}

fn find_callback_bound_to_destructor(
    callbacks: &[Trampoline],
    destroy: &mut Trampoline,
    destroy_index: usize,
) -> bool {
    for call in callbacks {
        if call.destroy_index == destroy_index {
            destroy.nullable = call.nullable;
            destroy.bound_name = call.bound_name.clone();
            return true;
        }
    }
    false
}

fn analyze_callbacks(
    env: &Env,
    func: &library::Function,
    cross_user_data_check: &mut HashMap<usize, usize>,
    user_data_indexes: &mut HashSet<usize>,
    parameters: &mut Parameters,
    used_types: &mut Vec<String>,
    bounds: &mut Bounds,
    to_glib_extras: &mut HashMap<usize, String>,
    imports: &mut Imports,
    destroys: &mut Vec<Trampoline>,
    callbacks: &mut Vec<Trampoline>,
    params: &mut Vec<library::Parameter>,
    configured_functions: &[&config::functions::Function],
    disable_length_detect: bool,
    in_trait: bool,
    commented: &mut bool,
    concurrency: library::Concurrency,
    type_tid: library::TypeId,
) {
    let mut to_replace = Vec::new();
    let mut to_remove = Vec::new();

    {
        // When closure data and destroy are specified in gir, they don't take into account the
        // actual closure parameter.
        let mut c_parameters = Vec::new();
        for (pos, par) in parameters.c_parameters.iter().enumerate() {
            if par.instance_parameter {
                continue;
            }
            c_parameters.push((par, pos));
        }

        let func_name = match &func.c_identifier {
            Some(n) => &n,
            None => &func.name,
        };
        let mut destructors_to_update = Vec::new();
        for pos in 0..parameters.c_parameters.len() {
            // If it is a user data parameter, we ignore it.
            if cross_user_data_check.values().any(|p| *p == pos) || user_data_indexes.contains(&pos)
            {
                continue;
            }
            let par = &parameters.c_parameters[pos];
            assert!(
                !par.instance_parameter || pos == 0,
                "Wrong instance parameter in {}",
                func.c_identifier.as_ref().unwrap()
            );
            if let Ok(rust_type) = RustType::builder(env, par.typ)
                .direction(par.direction)
                .try_from_glib(&par.try_from_glib)
                .try_build()
            {
                used_types.extend(rust_type.into_used_types());
            }
            let rust_type = env.library.type_(par.typ);
            let callback_info = if !*par.nullable || !rust_type.is_function() {
                let (to_glib_extra, callback_info) = bounds.add_for_parameter(
                    env,
                    func,
                    par,
                    false,
                    concurrency,
                    configured_functions,
                );
                if let Some(to_glib_extra) = to_glib_extra {
                    if par.c_type != "GDestroyNotify" {
                        to_glib_extras.insert(pos, to_glib_extra);
                    }
                }
                callback_info
            } else {
                None
            };

            if rust_type.is_function() {
                if par.c_type != "GDestroyNotify" {
                    if let Some((mut callback, destroy_index)) = analyze_callback(
                        func_name,
                        type_tid,
                        env,
                        &par,
                        &callback_info,
                        commented,
                        imports,
                        &c_parameters,
                        &rust_type,
                    ) {
                        if let Some(destroy_index) = destroy_index {
                            let user_data = cross_user_data_check
                                .entry(destroy_index)
                                .or_insert_with(|| callback.user_data_index);
                            if *user_data != callback.user_data_index {
                                warn_main!(
                                    type_tid,
                                    "`{}`: Different destructors cannot share the same user data",
                                    func_name
                                );
                                *commented = true;
                            }
                            callback.destroy_index = destroy_index;
                        } else {
                            user_data_indexes.insert(callback.user_data_index);
                            to_remove.push(callback.user_data_index);
                        }
                        callbacks.push(callback);
                        to_replace.push((pos, par.typ));
                        continue;
                    }
                } else if let Some((mut callback, _)) = analyze_callback(
                    func_name,
                    type_tid,
                    env,
                    &par,
                    &callback_info,
                    commented,
                    imports,
                    &c_parameters,
                    &rust_type,
                ) {
                    // We just assume that for API "cleaness", the destroy callback will always
                    // be |-> *after* <-| the initial callback.
                    if let Some(user_data_index) = cross_user_data_check.get(&pos) {
                        callback.user_data_index = *user_data_index;
                        callback.destroy_index = pos;
                    } else {
                        warn_main!(
                            type_tid,
                            "`{}`: no user data point to the destroy callback",
                            func_name,
                        );
                        *commented = true;
                    }
                    // We check if the user trampoline is there. If so, we change the destroy
                    // nullable value if needed.
                    if !find_callback_bound_to_destructor(&callbacks, &mut callback, pos) {
                        // Maybe the linked callback is after so we store it just in case...
                        destructors_to_update.push((pos, destroys.len()));
                    }
                    destroys.push(callback);
                    to_remove.push(pos);
                    continue;
                }
            }
            if !*commented {
                *commented |= RustType::builder(env, par.typ)
                    .direction(par.direction)
                    .scope(par.scope)
                    .try_from_glib(&par.try_from_glib)
                    .try_build_param()
                    .is_err();
            }
        }
        for (destroy_index, pos_in_destroys) in destructors_to_update {
            if !find_callback_bound_to_destructor(
                &callbacks,
                &mut destroys[pos_in_destroys],
                destroy_index,
            ) {
                warn_main!(
                    type_tid,
                    "`{}`: destructor without linked callback",
                    func_name
                );
            }
        }
    }

    // Check for cross "user data".
    if cross_user_data_check
        .values()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|a| a[0] == a[1])
    {
        *commented = true;
        warn_main!(
            type_tid,
            "`{}`: Different user data share the same destructors",
            func.name
        );
    }

    if !destroys.is_empty() || !callbacks.is_empty() {
        for (pos, typ) in to_replace {
            let ty = env.library.type_(typ);
            params[pos].typ = typ;
            params[pos].c_type = ty.get_glib_name().unwrap().to_owned();
        }
        let mut s = to_remove
            .iter()
            .chain(cross_user_data_check.values())
            .collect::<HashSet<_>>() // To prevent duplicates.
            .into_iter()
            .collect::<Vec<_>>();
        s.sort(); // We need to sort the array, otherwise the indexes won't be working
                  // anymore.
        for pos in s.iter().rev() {
            params.remove(**pos);
        }
        *parameters = function_parameters::analyze(
            env,
            &params,
            configured_functions,
            disable_length_detect,
            false,
            in_trait,
        );
    } else {
        warn_main!(
            type_tid,
            "`{}`: this is supposed to be a callback function but no callback was found...",
            func.name
        );
        *commented = true;
    }
}

fn analyze_function(
    env: &Env,
    obj: &config::gobjects::GObject,
    func_name: &str,
    name: String,
    status: GStatus,
    func: &library::Function,
    type_tid: library::TypeId,
    in_trait: bool,
    is_boxed: bool,
    configured_functions: &[&config::functions::Function],
    imports: &mut Imports,
) -> Info {
    let r#async = func.parameters.iter().any(|parameter| {
        parameter.scope == ParameterScope::Async && parameter.c_type == "GAsyncReadyCallback"
    });
    let has_callback_parameter = !r#async
        && func
            .parameters
            .iter()
            .any(|par| env.library.type_(par.typ).is_function());
    let concurrency = match env.library.type_(type_tid) {
        library::Type::Class(_) | library::Type::Interface(_) | library::Type::Record(_) => {
            obj.concurrency
        }
        _ => library::Concurrency::SendSync,
    };

    let mut commented = false;
    let mut bounds: Bounds = Default::default();
    let mut to_glib_extras = HashMap::<usize, String>::new();
    let mut used_types: Vec<String> = Vec::with_capacity(4);
    let mut trampoline = None;
    let mut callbacks = Vec::new();
    let mut destroys = Vec::new();
    let mut async_future = None;

    if !r#async
        && !has_callback_parameter
        && func
            .parameters
            .iter()
            .any(|par| par.c_type == "GDestroyNotify")
    {
        // In here, We have a DestroyNotify callback but no other callback is provided. A good
        // example of this situation is this function:
        // https://developer.gnome.org/gio/stable/GTlsPassword.html#g-tls-password-set-value-full
        warn_main!(
            type_tid,
            "Function \"{}\" with destroy callback without callbacks",
            func.name
        );
        commented = true;
    }

    let mut new_name = configured_functions.iter().find_map(|f| f.rename.clone());
    let is_constructor = configured_functions.iter().find_map(|f| f.is_constructor);

    let bypass_auto_rename = configured_functions.iter().any(|f| f.bypass_auto_rename);
    let is_constructor = is_constructor.unwrap_or(false);
    if !bypass_auto_rename && new_name.is_none() {
        if func.kind == library::FunctionKind::Constructor || is_constructor {
            if func.kind == library::FunctionKind::Constructor && is_constructor {
                warn_main!(
                    type_tid,
                    "`{}`: config forces 'constructor' on an already gir-annotated 'constructor'",
                    func_name
                );
            }

            if name.starts_with("new_from")
                || name.starts_with("new_with")
                || name.starts_with("new_for")
            {
                new_name = Some(name[4..].to_string());
            }
        } else {
            let nb_in_params = func
                .parameters
                .iter()
                .filter(|param| library::ParameterDirection::In == param.direction)
                .fold(0, |acc, _| acc + 1);
            let is_bool_getter = (func.parameters.len() == nb_in_params)
                && (func.ret.typ == library::TypeId::tid_bool());
            new_name = getter_rules::try_rename_would_be_getter(&name, is_bool_getter)
                .ok()
                .map(getter_rules::NewName::unwrap);
        }
    }

    let version = configured_functions
        .iter()
        .filter_map(|f| f.version)
        .min()
        .or(func.version);

    let version = env.config.filter_version(version);
    let deprecated_version = func.deprecated_version;
    let cfg_condition = configured_functions
        .iter()
        .find_map(|f| f.cfg_condition.clone());
    let doc_hidden = configured_functions.iter().any(|f| f.doc_hidden);
    let doc_ignore_parameters = configured_functions
        .iter()
        .find(|f| !f.doc_ignore_parameters.is_empty())
        .map(|f| f.doc_ignore_parameters.clone())
        .unwrap_or_default();
    let disable_length_detect = configured_functions.iter().any(|f| f.disable_length_detect);
    let no_future = configured_functions.iter().any(|f| f.no_future);
    let unsafe_ = configured_functions.iter().any(|f| f.unsafe_);
    let assertion = configured_functions.iter().find_map(|f| f.assertion);

    let imports = &mut imports.with_defaults(version, &cfg_condition);

    let ret = return_value::analyze(
        env,
        obj,
        func,
        type_tid,
        configured_functions,
        &mut used_types,
        imports,
    );
    commented |= ret.commented;

    let mut params = func.parameters.clone();
    let mut parameters = function_parameters::analyze(
        env,
        &params,
        configured_functions,
        disable_length_detect,
        r#async,
        in_trait,
    );
    parameters.analyze_return(env, &ret.parameter);

    if let Some(ref f) = ret.parameter {
        if let Type::Function(_) = env.library.type_(f.lib_par.typ) {
            if env.config.work_mode.is_normal() {
                warn!("Function \"{}\" returns callback", func.name);
                commented = true;
            }
        }
    }

    fixup_special_functions(env, name.as_str(), type_tid, is_boxed, &mut parameters);

    // Key: destroy callback index
    // Value: associated user data index
    let mut cross_user_data_check: HashMap<usize, usize> = HashMap::new();
    let mut user_data_indexes: HashSet<usize> = HashSet::new();

    if status.need_generate() {
        if !has_callback_parameter {
            for (pos, par) in parameters.c_parameters.iter().enumerate() {
                // FIXME: It'd be better if we assumed that user data wasn't gpointer all the time so
                //        we could handle it more generically.
                if r#async && is_gpointer(&par.c_type) {
                    continue;
                }
                assert!(
                    !par.instance_parameter || pos == 0,
                    "Wrong instance parameter in {}",
                    func.c_identifier.as_ref().unwrap()
                );
                if let Ok(rust_type) = RustType::builder(env, par.typ)
                    .direction(par.direction)
                    .try_from_glib(&par.try_from_glib)
                    .try_build()
                {
                    if !rust_type.as_str().ends_with("GString") || par.c_type == "gchar***" {
                        used_types.extend(rust_type.into_used_types());
                    }
                }

                let (to_glib_extra, callback_info) = bounds.add_for_parameter(
                    env,
                    func,
                    par,
                    r#async,
                    library::Concurrency::None,
                    configured_functions,
                );
                if let Some(to_glib_extra) = to_glib_extra {
                    to_glib_extras.insert(pos, to_glib_extra);
                }

                analyze_async(
                    env,
                    func,
                    type_tid,
                    new_name.as_ref().unwrap_or(&name),
                    callback_info,
                    &mut commented,
                    &mut trampoline,
                    no_future,
                    &mut async_future,
                    configured_functions,
                    &parameters,
                );
                let type_error = !(r#async
                    && *env.library.type_(par.typ)
                        == Type::Fundamental(library::Fundamental::Pointer))
                    && RustType::builder(env, par.typ)
                        .direction(par.direction)
                        .scope(par.scope)
                        .try_from_glib(&par.try_from_glib)
                        .try_build_param()
                        .is_err();
                if type_error {
                    commented = true;
                }
            }
            if r#async && trampoline.is_none() {
                commented = true;
            }
        } else {
            analyze_callbacks(
                env,
                func,
                &mut cross_user_data_check,
                &mut user_data_indexes,
                &mut parameters,
                &mut used_types,
                &mut bounds,
                &mut to_glib_extras,
                imports,
                &mut destroys,
                &mut callbacks,
                &mut params,
                configured_functions,
                disable_length_detect,
                in_trait,
                &mut commented,
                concurrency,
                type_tid,
            );
        }
    }

    for par in &parameters.rust_parameters {
        // Disallow fundamental arrays without length
        let is_len_for_par = |t: &Transformation| {
            if let TransformationType::Length { ref array_name, .. } = t.transformation_type {
                array_name == &par.name
            } else {
                false
            }
        };
        if is_carray_with_direct_elements(env, par.typ)
            && !parameters.transformations.iter().any(is_len_for_par)
        {
            commented = true;
        }
    }

    let (outs, unsupported_outs) = out_parameters::analyze(
        env,
        func,
        &parameters.c_parameters,
        &ret,
        configured_functions,
    );
    if unsupported_outs {
        warn_main!(
            type_tid,
            "Function {} has unsupported outs",
            func.c_identifier.as_ref().unwrap_or(&func.name)
        );
        commented = true;
    } else if status.need_generate() && !commented {
        if !outs.is_empty() {
            out_parameters::analyze_imports(env, &func.parameters, imports);
        }
        if let Some(AsyncTrampoline {
            ref output_params, ..
        }) = trampoline
        {
            out_parameters::analyze_imports(
                env,
                output_params.iter().map(|out| &out.lib_par),
                imports,
            );
        }
    }

    if r#async && status.need_generate() && !commented {
        imports.add("std::ptr");
        imports.add("std::boxed::Box as Box_");
        imports.add("std::pin::Pin");

        if let Some(ref trampoline) = trampoline {
            for out in &trampoline.output_params {
                if let Ok(rust_type) = RustType::builder(env, out.lib_par.typ)
                    .direction(ParameterDirection::Out)
                    .try_build()
                {
                    used_types.extend(rust_type.into_used_types());
                }
            }
            if let Some(ref out) = trampoline.ffi_ret {
                if let Ok(rust_type) = RustType::builder(env, out.lib_par.typ)
                    .direction(ParameterDirection::Return)
                    .try_build()
                {
                    used_types.extend(rust_type.into_used_types());
                }
            }
        }
    }

    if status.need_generate() && !commented {
        if (!destroys.is_empty() || !callbacks.is_empty())
            && callbacks.iter().any(|c| !c.scope.is_call())
        {
            imports.add("std::boxed::Box as Box_");
        }

        for transformation in &mut parameters.transformations {
            if let Some(to_glib_extra) = to_glib_extras.get(&transformation.ind_c) {
                transformation
                    .transformation_type
                    .set_to_glib_extra(to_glib_extra);
            }
        }

        imports.add_used_types(&used_types);
        if ret.base_tid.is_some() {
            imports.add("glib::object::Cast");
        }

        if func.name.parse::<special_functions::Type>().is_err() {
            imports.add("glib::translate::*");
        }
        bounds.update_imports(imports);
    }

    let visibility = if commented {
        Visibility::Comment
    } else {
        Visibility::Public
    };
    let is_method = func.kind == library::FunctionKind::Method;
    let assertion =
        assertion.unwrap_or_else(|| SafetyAssertionMode::of(env, is_method, &parameters));

    Info {
        name,
        func_name: func_name.to_string(),
        new_name,
        glib_name: func.c_identifier.as_ref().unwrap().clone(),
        status,
        kind: func.kind,
        visibility,
        type_name: RustType::try_new(env, type_tid),
        parameters,
        ret,
        bounds,
        outs,
        version,
        deprecated_version,
        not_version: None,
        cfg_condition,
        assertion,
        doc_hidden,
        doc_ignore_parameters,
        r#async,
        unsafe_,
        trampoline,
        async_future,
        callbacks,
        destroys,
        remove_params: cross_user_data_check.values().cloned().collect::<Vec<_>>(),
    }
}

pub fn is_carray_with_direct_elements(env: &Env, typ: library::TypeId) -> bool {
    match *env.library.type_(typ) {
        Type::CArray(inner_tid) => {
            use super::conversion_type::ConversionType;
            match env.library.type_(inner_tid) {
                Type::Fundamental(..)
                    if ConversionType::of(env, inner_tid) == ConversionType::Direct =>
                {
                    true
                }
                _ => false,
            }
        }
        _ => false,
    }
}

fn analyze_async(
    env: &Env,
    func: &library::Function,
    type_tid: library::TypeId,
    codegen_name: &str,
    callback_info: Option<CallbackInfo>,
    commented: &mut bool,
    trampoline: &mut Option<AsyncTrampoline>,
    no_future: bool,
    async_future: &mut Option<AsyncFuture>,
    configured_functions: &[&config::functions::Function],
    parameters: &function_parameters::Parameters,
) -> bool {
    if let Some(CallbackInfo {
        callback_type,
        success_parameters,
        error_parameters,
        bound_name,
    }) = callback_info
    {
        // Checks for /*Ignored*/ or other error comments
        *commented |= callback_type.contains("/*");
        let func_name = func.c_identifier.as_ref().unwrap();
        let finish_func_name = finish_function_name(func_name);
        let mut output_params = vec![];
        let mut ffi_ret = None;
        if let Some(function) = find_function(env, &finish_func_name) {
            if use_function_return_for_result(
                env,
                function.ret.typ,
                &func.name,
                configured_functions,
            ) {
                ffi_ret = Some(analysis::Parameter::from_return_value(
                    env,
                    &function.ret,
                    &configured_functions,
                ));
            }

            for param in &function.parameters {
                let mut lib_par = param.clone();
                if nameutil::needs_mangling(&param.name) {
                    lib_par.name = nameutil::mangle_keywords(&*param.name).into_owned();
                }
                let configured_parameters = configured_functions.matched_parameters(&lib_par.name);
                output_params.push(analysis::Parameter::from_parameter(
                    env,
                    &lib_par,
                    &configured_parameters,
                ));
            }
        }
        if trampoline.is_some() || async_future.is_some() {
            warn_main!(
                type_tid,
                "{}: Cannot handle callbacks and async parameters at the same time for the \
                 moment",
                func.name
            );
            *commented = true;
            return false;
        }
        if !*commented && (success_parameters.is_empty() || error_parameters.is_empty()) {
            if success_parameters.is_empty() {
                warn_main!(
                    type_tid,
                    "{}: missing success parameters for async future",
                    func.name
                );
            } else if error_parameters.is_empty() {
                warn_main!(
                    type_tid,
                    "{}: missing error parameters for async future",
                    func.name
                );
            }
            *commented = true;
            return false;
        }
        let is_method = func.kind == FunctionKind::Method;

        *trampoline = Some(AsyncTrampoline {
            is_method,
            name: format!("{}_trampoline", codegen_name),
            finish_func_name: format!("{}::{}", env.main_sys_crate_name(), finish_func_name),
            callback_type,
            bound_name,
            output_params,
            ffi_ret,
        });

        if !no_future {
            *async_future = Some(AsyncFuture {
                is_method,
                name: format!("{}_future", codegen_name),
                success_parameters,
                error_parameters,
                assertion: match SafetyAssertionMode::of(env, is_method, &parameters) {
                    SafetyAssertionMode::None => SafetyAssertionMode::None,
                    // "_future" functions calls the "async" one which has the init check, so no
                    // need to do it twice.
                    _ => SafetyAssertionMode::Skip,
                },
            });
        }
        true
    } else {
        false
    }
}

fn analyze_callback(
    func_name: &str,
    type_tid: library::TypeId,
    env: &Env,
    par: &CParameter,
    callback_info: &Option<CallbackInfo>,
    commented: &mut bool,
    imports: &mut Imports,
    c_parameters: &[(&CParameter, usize)],
    rust_type: &Type,
) -> Option<(Trampoline, Option<usize>)> {
    let mut imports_to_add = Vec::new();

    if let Type::Function(func) = rust_type {
        if par.c_type != "GDestroyNotify" {
            if let Some(user_data) = par.user_data_index {
                if user_data >= c_parameters.len() {
                    warn_main!(type_tid,
                               "function `{}` has an invalid user data index of {} when there are {} parameters",
                               func_name,
                               user_data,
                               c_parameters.len());
                    return None;
                } else if !is_gpointer(&c_parameters[user_data].0.c_type) {
                    *commented = true;
                    warn_main!(
                        type_tid,
                        "function `{}`'s callback `{}` has invalid user data",
                        func_name,
                        par.name
                    );
                    return None;
                }
            } else {
                *commented = true;
                warn_main!(
                    type_tid,
                    "function `{}`'s callback `{}` without associated user data",
                    func_name,
                    par.name
                );
                return None;
            }
            if let Some(destroy_index) = par.destroy_index {
                if destroy_index >= c_parameters.len() {
                    warn_main!(
                        type_tid,
                        "function `{}` has an invalid destroy index of {} when there are {} \
                         parameters",
                        func_name,
                        destroy_index,
                        c_parameters.len()
                    );
                    return None;
                }
                if c_parameters[destroy_index].0.c_type != "GDestroyNotify" {
                    *commented = true;
                    warn_main!(
                        type_tid,
                        "function `{}`'s callback `{}` has invalid destroy callback",
                        func_name,
                        par.name
                    );
                    return None;
                }
            }
        }

        // If we don't have a "user data" parameter, we can't get the closure so there's nothing we
        // can do...
        if par.c_type != "GDestroyNotify"
            && (func.parameters.is_empty() || !func.parameters.iter().any(|c| c.closure.is_some()))
        {
            *commented = true;
            warn_main!(
                type_tid,
                "Closure type `{}` doesn't provide user data",
                par.c_type
            );
            return None;
        }

        let parameters =
            crate::analysis::trampoline_parameters::analyze(env, &func.parameters, par.typ, &[]);
        if par.c_type != "GDestroyNotify" && !*commented {
            *commented |= func.parameters.iter().any(|p| {
                if p.closure.is_none() {
                    crate::analysis::trampolines::type_error(env, p).is_some()
                } else {
                    false
                }
            });
        }
        for p in parameters.rust_parameters.iter() {
            if let Ok(rust_type) = RustType::builder(env, p.typ)
                .direction(p.direction)
                .try_from_glib(&p.try_from_glib)
                .try_build()
            {
                imports_to_add.extend(rust_type.into_used_types());
            }
        }
        if let Ok(rust_type) = RustType::builder(env, func.ret.typ)
            .direction(ParameterDirection::Return)
            .try_build()
        {
            if !rust_type.as_str().ends_with("GString") {
                imports_to_add.extend(rust_type.into_used_types());
            }
        }
        let user_data_index = par.user_data_index.unwrap_or(0);
        if par.c_type != "GDestroyNotify" && c_parameters.len() <= user_data_index {
            warn_main!(
                type_tid,
                "`{}`: Invalid user data index of `{}`",
                func.name,
                user_data_index
            );
            *commented = true;
            None
        } else if match par.destroy_index {
            Some(destroy_index) => c_parameters.len() <= destroy_index,
            None => false,
        } {
            warn_main!(
                type_tid,
                "`{}`: Invalid destroy index of `{}`",
                func.name,
                par.destroy_index.unwrap()
            );
            *commented = true;
            None
        } else {
            if !*commented {
                for import in imports_to_add {
                    imports.add_used_type(&import);
                }
            }
            Some((
                Trampoline {
                    name: par.name.to_string(),
                    parameters,
                    ret: func.ret.clone(),
                    bound_name: match callback_info {
                        Some(x) => x.bound_name.to_string(),
                        None => match RustType::builder(env, par.typ)
                            .direction(par.direction)
                            .nullable(par.nullable)
                            .scope(par.scope)
                            .try_build()
                        {
                            Ok(rust_type) => rust_type.into_string(),
                            Err(_) => {
                                warn_main!(type_tid, "`{}`: unknown type", func.name);
                                return None;
                            }
                        },
                    },
                    bounds: Bounds::default(),
                    version: None,
                    inhibit: false,
                    concurrency: library::Concurrency::None,
                    is_notify: false,
                    scope: par.scope,
                    // If destroy callback, id doesn't matter.
                    user_data_index: if par.c_type != "GDestroyNotify" {
                        c_parameters[user_data_index].1
                    } else {
                        0
                    },
                    destroy_index: 0,
                    nullable: par.nullable,
                    type_name: env.library.type_(type_tid).get_name(),
                },
                par.destroy_index
                    .map(|destroy_index| c_parameters[destroy_index].1),
            ))
        }
    } else {
        None
    }
}

pub fn find_function<'a>(env: &'a Env, c_identifier: &str) -> Option<&'a Function> {
    let find = |functions: &'a [Function]| -> Option<&'a Function> {
        for function in functions {
            if let Some(ref func_c_identifier) = function.c_identifier {
                if func_c_identifier == c_identifier {
                    return Some(function);
                }
            }
        }
        None
    };

    if let Some(index) = env.library.find_namespace(&env.config.library_name) {
        let namespace = env.library.namespace(index);
        if let Some(f) = find(&namespace.functions) {
            return Some(f);
        }
        for typ in &namespace.types {
            if let Some(Type::Class(class)) = typ {
                if let Some(f) = find(&class.functions) {
                    return Some(f);
                }
            } else if let Some(Type::Interface(interface)) = typ {
                if let Some(f) = find(&interface.functions) {
                    return Some(f);
                }
            }
        }
    }
    None
}

/// Given async function name tries to guess the name of finish function.
pub fn finish_function_name(mut func_name: &str) -> String {
    if func_name.ends_with("_async") {
        let len = func_name.len() - "_async".len();
        func_name = &func_name[0..len];
    }
    format!("{}_finish", &func_name)
}

pub fn find_index_to_ignore<'a>(
    parameters: impl IntoIterator<Item = &'a library::Parameter>,
    ret: Option<&'a library::Parameter>,
) -> Option<usize> {
    parameters
        .into_iter()
        .chain(ret)
        .find(|param| param.array_length.is_some())
        .and_then(|param| param.array_length.map(|length| length as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finish_function_name() {
        assert_eq!(
            "g_file_copy_finish",
            &finish_function_name("g_file_copy_async")
        );
        assert_eq!("g_bus_get_finish", &finish_function_name("g_bus_get"));
    }
}
