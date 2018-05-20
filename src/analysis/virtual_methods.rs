use std::collections::HashMap;
use std::vec::Vec;
use std::borrow::Borrow;

use config;
use config::gobjects::GObject;
use env::Env;
use nameutil;
use super::trampolines;
use traits::*;
use version::Version;

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
use library::{self, Signal, Function, FunctionKind, Nullable, Parameter, ParameterScope, Type};

use super::functions::{
    Visibility,
    AsyncFuture,
    AsyncTrampoline,
    is_carray_with_direct_elements,
    finish_function_name,
    find_function
};



#[derive(Debug)]
pub struct Info {
    pub name: String,
    pub glib_name: String,
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
    pub async_future: Option<AsyncFuture>,
}


pub fn analyze(
    env: &Env,
    methods: &[library::Function],
    type_tid: library::TypeId,
    in_trait: bool,
    obj: &GObject,
    imports: &mut Imports,
    mut signatures: Option<&mut Signatures>,
    deps: Option<&[library::TypeId]>,
) -> Vec<Info> {
    let mut vmethds = Vec::new();

    for method in methods {
        let method = method.borrow();
        let configured_methods = obj.virtual_methods.matched(&method.name);
        if configured_methods.iter().any(|m| m.ignore) {
            continue;
        }
        if env.is_totally_deprecated(method.deprecated_version) {
            continue;
        }

        let name = nameutil::mangle_keywords(&*method.name).into_owned();
        let signature_params = Signature::new(method);
        let mut not_version = None;
        if let Some(deps) = deps {
            let (has, version) = signature_params.has_in_deps(env, &name, deps);
            if has {
                match version {
                    Some(v) if v > env.config.min_cfg_version => not_version = version,
                    _ => continue,
                }
            }
        }

        if let Some(ref mut signatures) = signatures {
            signatures.insert(name.clone(), signature_params);
        }

        let mut info = analyze_virtual_method(env, name, method, type_tid, in_trait,
                                              &configured_methods, obj, imports);
        info.not_version = not_version;
        vmethds.push(info);
    }

    vmethds
}



fn analyze_virtual_method(
    env: &Env,
    name: String,
    method: &library::Function,
    type_tid: library::TypeId,
    in_trait: bool,
    configured_methods: &[&config::functions::Function],
    obj: &GObject,
    imports: &mut Imports
) -> Info {

    let async = method.parameters.iter().any(|parameter| parameter.scope == ParameterScope::Async);
    let mut commented = false;
    let mut bounds: Bounds = Default::default();
    let mut to_glib_extras = HashMap::<usize, String>::new();
    let mut used_types: Vec<String> = Vec::with_capacity(4);
    let mut trampoline = None;
    let mut async_future = None;

    let version = configured_methods
        .iter()
        .filter_map(|f| f.version)
        .min()
        .or(method.version);
    let version = env.config.filter_version(version);
    let deprecated_version = method.deprecated_version;
    let cfg_condition = configured_methods
        .iter()
        .filter_map(|f| f.cfg_condition.clone())
        .next();
    let doc_hidden = configured_methods.iter().any(|f| f.doc_hidden);
    let disable_length_detect = configured_methods.iter().any(|f| f.disable_length_detect);

    let ret = return_value::analyze(
        env,
        method,
        type_tid,
        configured_methods,
        &mut used_types,
        imports,
    );
    commented |= ret.commented;

    let mut parameters = function_parameters::analyze(
        env,
        &method.parameters,
        configured_methods,
        disable_length_detect,
        async,
        in_trait
    );
    parameters.analyze_return(env, &ret.parameter);

    for (pos, par) in parameters.c_parameters.iter().enumerate() {
        assert!(
            !par.instance_parameter || pos == 0,
            "Wrong instance parameter in {}",
            method.c_identifier.as_ref().unwrap()
        );
        if let Ok(s) = used_rust_type(env, par.typ) {
            used_types.push(s);
        }
        let (to_glib_extra, type_string) = bounds.add_for_parameter(env, method, par, async);
        if let Some(to_glib_extra) = to_glib_extra {
            to_glib_extras.insert(pos, to_glib_extra);
        }
        if let Some((callback_type, success_parameters, error_parameters, bound_name)) = type_string {
            // Checks for /*Ignored*/ or other error comments
            if callback_type.find("/*").is_some() {
                commented = true;
            }
            let func_name = method.c_identifier.as_ref().unwrap();
            let finish_func_name = finish_function_name(func_name);
            let mut output_params = vec![];
            let mut ffi_ret = None;
            if let Some(function) = find_function(env, &finish_func_name) {
                if use_function_return_for_result(env, function.ret.typ) {
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
                is_method: true,
                name: format!("{}_trampoline", method.name),
                finish_func_name,
                callback_type,
                bound_name,
                output_params,
                ffi_ret,
            });

            async_future = Some(AsyncFuture {
                is_method: true,
                name: format!("{}_future", method.name),
                success_parameters,
                error_parameters,
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

    if async && trampoline.is_none() {
        commented = true;
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

    let (outs, unsupported_outs) = out_parameters::analyze(
        env,
        method,
        &parameters.c_parameters,
        &ret,
        configured_methods,
    );
    if unsupported_outs {
        warn!(
            "Function {} has unsupported outs",
            method.c_identifier.as_ref().unwrap_or(&method.name)
        );
        commented = true;
    } else if !outs.is_empty() && !commented {
        out_parameters::analyze_imports(env, method, imports);
    }

    if async && !commented {
        if env.config.library_name != "Gio" {
            imports.add("gio_ffi", version);
            imports.add_with_constraint("gio", version, Some("futures"));
        }
        imports.add("glib_ffi", None);
        imports.add("gobject_ffi", None);
        imports.add_with_constraint("futures_core", version, Some("futures"));
        imports.add_with_constraint("std::boxed::Box as Box_", version, Some("futures"));

        if let Some(ref trampoline) = trampoline {
            for par in &trampoline.output_params {
                if let Ok(s) = used_rust_type(env, par.typ) {
                    used_types.push(s);
                }
            }
            if let Some(ref par) = trampoline.ffi_ret {
                if let Ok(s) = used_rust_type(env, par.typ) {
                    used_types.push(s);
                }
            }
        }
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

    let visibility = if commented {
        Visibility::Comment
    } else {
        Visibility::Public
    };
    let assertion = SafetyAssertionMode::of(env, true, &parameters);

    Info {
        name,
        glib_name: method.c_identifier.as_ref().unwrap().clone(),
        visibility,
        type_name: rust_type(env, type_tid),
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
        async,
        trampoline,
        async_future,
    }
}


//     let mut used_types: Vec<String> = Vec::with_capacity(4);
//     let version = configured_signals
//         .iter()
//         .filter_map(|f| f.version)
//         .min()
//         .or(signal.version);
//     let deprecated_version = signal.deprecated_version;
//     let doc_hidden = configured_signals.iter().any(|f| f.doc_hidden);
//
//     let connect_name = format!("connect_{}", nameutil::signal_to_snake(&signal.name));
//     let trampoline_name = trampolines::analyze(
//         env,
//         signal,
//         type_tid,
//         in_trait,
//         configured_signals,
//         trampolines,
//         obj,
//         &mut used_types,
//         version,
//     );
//
//     let action_emit_name = if signal.is_action {
//         if !in_trait {
//             imports.add("glib::object::ObjectExt", version);
//         }
//         Some(format!("emit_{}", nameutil::signal_to_snake(&signal.name)))
//     } else {
//         None
//     };
//
//     if trampoline_name.is_ok() {
//         imports.add_used_types(&used_types, version);
//         if in_trait {
//             imports.add("glib", version);
//             imports.add("glib::object::Downcast", version);
//         }
//         imports.add("glib::signal::connect", version);
//         imports.add("glib::signal::SignalHandlerId", version);
//         imports.add("std::mem::transmute", version);
//         imports.add("std::boxed::Box as Box_", version);
//         imports.add("glib_ffi", version);
//     }
//
//     let info = Info {
//         connect_name,
//         signal_name: signal.name.clone(),
//         trampoline_name,
//         action_emit_name,
//         version,
//         deprecated_version,
//         doc_hidden,
//     };
//     Some(info)
// }
