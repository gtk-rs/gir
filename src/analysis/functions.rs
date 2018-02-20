/*
 * TODO: better heuristic (https://bugzilla.gnome.org/show_bug.cgi?id=623635#c5)
 * TODO: ProgressCallback types (not specific to async).
 * TODO: add annotation for methods like g_file_replace_contents_bytes_async where the finish
 * method has a different prefix.
 */

use std::collections::HashMap;
use std::vec::Vec;

use analysis::bounds::Bounds;
use analysis::function_parameters::{self, Parameters, Transformation, TransformationType};
use analysis::out_parameters::use_function_return_for_result;
use analysis::imports::Imports;
use analysis::out_parameters;
use analysis::ref_mode::RefMode;
use analysis::return_value;
use analysis::rust_type::*;
use analysis::safety_assertion_mode::SafetyAssertionMode;
use analysis::signatures::{Signature, Signatures};
use config;
use env::Env;
use library::{self, Function, FunctionKind, Nullable, Parameter, Type};
use nameutil;
use traits::*;
use version::Version;
use std::borrow::Borrow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Comment,
    Private,
    Hidden,
}

impl Visibility {
    pub fn hidden(&self) -> bool {
        *self == Visibility::Hidden
    }
}

#[derive(Clone, Debug)]
pub struct AsyncTrampoline {
    pub is_method: bool,
    pub name: String,
    pub finish_func_name: String,
    pub callback_type: String,
    pub bound_name: char,
    pub output_params: Vec<Parameter>,
    pub ffi_ret: Option<Parameter>,
}

#[derive(Debug)]
pub struct Info {
    pub name: String,
    pub glib_name: String,
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
    pub async: bool,
    pub trampoline: Option<AsyncTrampoline>,
}

impl Info {
    pub fn is_async_finish(&self, env: &Env) -> bool {
        let has_async_result = self.parameters.rust_parameters.iter()
            .any(|param| param.typ.full_name(&env.library) == "Gio.AsyncResult");
        self.name.ends_with("_finish") && has_async_result
    }
}

pub fn analyze<F: Borrow<library::Function>>(
    env: &Env,
    functions: &[F],
    type_tid: library::TypeId,
    in_trait: bool,
    obj: &config::gobjects::GObject,
    imports: &mut Imports,
    mut signatures: Option<&mut Signatures>,
    deps: Option<&[library::TypeId]>,
) -> Vec<Info> {
    let mut funcs = Vec::new();

    for func in functions {
        let func = func.borrow();
        let configured_functions = obj.functions.matched(&func.name);
        if configured_functions.iter().any(|f| f.ignore) {
            continue;
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
        if let Some(ref mut signatures) = signatures {
            signatures.insert(name.clone(), signature_params);
        }

        let mut info = analyze_function(env, name, func, type_tid, in_trait,
                                        &configured_functions, imports);
        info.not_version = not_version;
        funcs.push(info);
    }

    funcs
}

fn analyze_function(
    env: &Env,
    name: String,
    func: &library::Function,
    type_tid: library::TypeId,
    in_trait: bool,
    configured_functions: &[&config::functions::Function],
    imports: &mut Imports,
) -> Info {
    let async = func.parameters.iter().any(|parameter| parameter.async) && name.ends_with("_async");
    let mut commented = false;
    let mut bounds: Bounds = Default::default();
    let mut to_glib_extras = HashMap::<usize, String>::new();
    let mut used_types: Vec<String> = Vec::with_capacity(4);
    let mut trampoline = None;

    let version = configured_functions
        .iter()
        .filter_map(|f| f.version)
        .min()
        .or(func.version);
    let version = env.config.filter_version(version);
    let deprecated_version = func.deprecated_version;
    let cfg_condition = configured_functions
        .iter()
        .filter_map(|f| f.cfg_condition.clone())
        .next();
    let doc_hidden = configured_functions.iter().any(|f| f.doc_hidden);
    let disable_length_detect = configured_functions.iter().any(|f| f.disable_length_detect);

    let ret = return_value::analyze(
        env,
        func,
        type_tid,
        configured_functions,
        &mut used_types,
        imports,
    );
    commented |= ret.commented;

    let mut parameters = function_parameters::analyze(
        env,
        &func.parameters,
        configured_functions,
        disable_length_detect,
        async,
        in_trait
    );
    parameters.analyze_return(env, &ret.parameter);

    for (pos, par) in parameters.c_parameters.iter().enumerate() {
        assert!(
            !par.instance_parameter || pos == 0,
            "Wrong instance parameter in {}",
            func.c_identifier.as_ref().unwrap()
        );
        if let Ok(s) = used_rust_type(env, par.typ) {
            used_types.push(s);
        }
        let (to_glib_extra, type_string) = bounds.add_for_parameter(env, func, par, async);
        if let Some(to_glib_extra) = to_glib_extra {
            to_glib_extras.insert(pos, to_glib_extra);
        }
        if let Some((callback_type, bound_name)) = type_string {
            let func_name = func.c_identifier.as_ref().unwrap();
            let finish_func_name = replace_async_by_finish(func_name);
            let mut output_params = vec![];
            let mut ffi_ret = None;
            if let Some(function) = find_function(env, &finish_func_name) {
                if use_function_return_for_result(env, &function.ret) {
                    ffi_ret = Some(function.ret.clone());
                }

                output_params.extend(function.parameters.clone());
                for param in &mut output_params {
                    if nameutil::needs_mangling(&param.name) {
                        param.name = nameutil::mangle_keywords(&*param.name).into_owned();
                    }
                }
            }
            trampoline = Some(AsyncTrampoline {
                is_method: func.kind == FunctionKind::Method,
                name: format!("{}_trampoline", func.name),
                finish_func_name,
                callback_type,
                bound_name,
                output_params,
                ffi_ret,
            });
        }
        let type_error =
            !(async && *env.library.type_(par.typ) == Type::Fundamental(library::Fundamental::Pointer)) &&
            parameter_rust_type(env, par.typ, par.direction, Nullable(false), RefMode::None)
                .is_err();
        if type_error {
            commented = true;
        }
    }

    for par in &parameters.rust_parameters {
        // Disallow fundamental arrays without length
        let is_len_for_par = |t: &&Transformation| {
            if let TransformationType::Length { ref array_name, .. } = t.transformation_type {
                array_name == &par.name
            } else {
                false
            }
        };
        if is_carray_with_direct_elements(env, par.typ)
            && parameters
                .transformations
                .iter()
                .find(is_len_for_par)
                .is_none()
        {
            commented = true;
        }
    }

    let (outs, unsupported_outs) = out_parameters::analyze(env, func, configured_functions);
    if unsupported_outs {
        warn!(
            "Function {} has unsupported outs",
            func.c_identifier.as_ref().unwrap_or(&func.name)
        );
        commented = true;
    } else if !outs.is_empty() && !commented {
        out_parameters::analyze_imports(env, func, imports);
    }

    if async && !commented && env.config.library_name != "Gio" {
        imports.add("gio_ffi", version);
    }

    if !commented {
        for transformation in &mut parameters.transformations {
            if let Some(to_glib_extra) = to_glib_extras.get(&transformation.ind_c) {
                transformation
                    .transformation_type
                    .set_to_glib_extra(to_glib_extra);
            }
        }

        imports.add_used_types(&used_types, version);
        if ret.base_tid.is_some() {
            imports.add("glib::object::Downcast", None);
        }
        bounds.update_imports(imports);
    }

    let visibility = if commented || (async && trampoline.is_none()) {
        Visibility::Comment
    } else {
        Visibility::Public
    };
    let is_method = func.kind == library::FunctionKind::Method;
    let assertion = SafetyAssertionMode::of(env, is_method, &parameters);

    Info {
        name: name,
        glib_name: func.c_identifier.as_ref().unwrap().clone(),
        kind: func.kind,
        visibility: visibility,
        type_name: rust_type(env, type_tid),
        parameters: parameters,
        ret: ret,
        bounds: bounds,
        outs: outs,
        version: version,
        deprecated_version: deprecated_version,
        not_version: None,
        cfg_condition: cfg_condition,
        assertion: assertion,
        doc_hidden: doc_hidden,
        async,
        trampoline,
    }
}

pub fn is_carray_with_direct_elements(env: &Env, typ: library::TypeId) -> bool {
    match *env.library.type_(typ) {
        Type::CArray(inner_tid) => {
            use super::conversion_type::ConversionType;
            match *env.library.type_(inner_tid) {
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

pub fn find_function<'a>(env: &'a Env, c_identifier: &str) -> Option<&'a Function> {
    if let Some(index) = env.library.find_namespace(&env.config.library_name) {
        let namespace = env.library.namespace(index);
        for typ in &namespace.types {
            if let Some(Type::Class(ref class)) = *typ {
                for function in &class.functions {
                    if let Some(ref func_c_identifier) = function.c_identifier {
                        if func_c_identifier == c_identifier {
                          return Some(function);
                        }
                    }
                }
            } else if let Some(Type::Interface(ref interface)) = *typ {
                for function in &interface.functions {
                    if let Some(ref func_c_identifier) = function.c_identifier {
                        if func_c_identifier == c_identifier {
                          return Some(function);
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn replace_async_by_finish(func_name: &str) -> String {
    let len = func_name.len() - "_async".len();
    format!("{}_finish", &func_name[0..len])
}

pub fn find_index_to_ignore(parameters: &[Parameter]) -> Option<usize> {
    parameters.iter()
        .find(|param| param.array_length.is_some())
        .and_then(|param| param.array_length.map(|length| length as usize))
}
