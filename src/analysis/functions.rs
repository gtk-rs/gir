use std::collections::HashMap;
use std::vec::Vec;

use analysis::bounds::Bounds;
use analysis::function_parameters::{self, Parameters, TransformationType};
use analysis::imports::Imports;
use analysis::out_parameters;
use analysis::ref_mode::RefMode;
use analysis::return_value;
use analysis::rust_type::*;
use analysis::safety_assertion_mode::SafetyAssertionMode;
use analysis::signatures::{Signature, Signatures};
use config;
use env::Env;
use library::{self, Nullable, Type};
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
    pub target_os: Option<String>,
    pub assertion: SafetyAssertionMode,
    pub doc_hidden: bool,
}

pub fn analyze<F: Borrow<library::Function>>(
    env: &Env,
    functions: &[F],
    type_tid: library::TypeId,
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

        let mut info = analyze_function(env, name, func, type_tid, &configured_functions, imports);
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
    configured_functions: &[&config::functions::Function],
    imports: &mut Imports,
) -> Info {
    let mut commented = false;
    let mut bounds: Bounds = Default::default();
    let mut to_glib_extras = HashMap::<usize, String>::new();
    let mut used_types: Vec<String> = Vec::with_capacity(4);

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
    let target_os = configured_functions
        .iter()
        .filter_map(|f| f.target_os.clone())
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
        if let Some(to_glib_extra) = bounds.add_for_parameter(env, func, par) {
            to_glib_extras.insert(pos, to_glib_extra);
        }
        let type_error =
            parameter_rust_type(env, par.typ, par.direction, Nullable(false), RefMode::None)
                .is_err();
        if type_error {
            commented = true;
        }
    }

    for par in &parameters.rust_parameters {
        // Disallow fundamental arrays without length
        if is_carray_with_direct_elements(env, par.typ) &&
            parameters
                .transformations
                .iter()
                .find(|t| {
                    if let TransformationType::Length { ref array_name, .. } =
                        t.transformation_type
                    {
                        array_name == &par.name
                    } else {
                        false
                    }
                })
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
        target_os: target_os,
        assertion: assertion,
        doc_hidden: doc_hidden,
    }
}

pub fn is_carray_with_direct_elements(env: &Env, typ: library::TypeId) -> bool {
    match *env.library.type_(typ) {
        Type::CArray(inner_tid) => {
            use super::conversion_type::ConversionType;
            match *env.library.type_(inner_tid) {
                Type::Fundamental(..)
                    if ConversionType::of(&env.library, inner_tid) == ConversionType::Direct =>
                {
                    true
                }
                _ => false,
            }
        }
        _ => false,
    }
}
