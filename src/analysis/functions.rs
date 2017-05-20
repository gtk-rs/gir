use std::vec::Vec;

use analysis::bounds::Bounds;
use analysis::imports::Imports;
use analysis::out_parameters;
use analysis::parameter;
use analysis::ref_mode::RefMode;
use analysis::return_value;
use analysis::rust_type::*;
use analysis::safety_assertion_mode::SafetyAssertionMode;
use analysis::signatures::{Signature, Signatures};
use config;
use env::Env;
use library::{self, Nullable};
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

//TODO: change use Parameter to reference?
#[derive(Debug)]
pub struct Info {
    pub name: String,
    pub glib_name: String,
    pub kind: library::FunctionKind,
    pub visibility: Visibility,
    pub type_name: Result,
    pub parameters: Vec<parameter::Parameter>,
    pub ret: return_value::Info,
    pub bounds: Bounds,
    pub outs: out_parameters::Info,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub not_version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub assertion: SafetyAssertionMode,
    pub doc_hidden: bool,
}

pub fn analyze<F: Borrow<library::Function>>(env: &Env, functions: &[F], type_tid: library::TypeId,
               obj: &config::gobjects::GObject, imports: &mut Imports,
               mut signatures: Option<&mut Signatures>,
               deps: Option<&[library::TypeId]>) -> Vec<Info> {
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
                        Some(v) if v > env.config.min_cfg_version =>
                            not_version = version,
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

fn analyze_function(env: &Env, name: String, func: &library::Function, type_tid: library::TypeId,
                    configured_functions: &[&config::functions::Function],
                    imports: &mut Imports) -> Info {
    let mut commented = false;
    let mut bounds: Bounds = Default::default();
    let mut used_types: Vec<String> = Vec::with_capacity(4);

    let version = configured_functions.iter().filter_map(|f| f.version).min()
        .or(func.version);
    let version = env.config.filter_version(version);
    let deprecated_version = func.deprecated_version;
    let cfg_condition = configured_functions.iter().filter_map(|f| f.cfg_condition.clone()).next();
    let doc_hidden = configured_functions.iter().any(|f| f.doc_hidden);

    let ret = return_value::analyze(env, func, type_tid, configured_functions, &mut used_types, imports);
    commented |= ret.commented;

    let mut parameters: Vec<parameter::Parameter> =
        func.parameters.iter().map(|par| parameter::analyze(env, par, configured_functions)).collect();

    for (pos, par) in parameters.iter_mut().enumerate() {
        assert!(!par.instance_parameter || pos == 0,
            "Wrong instance parameter in {}", func.c_identifier.as_ref().unwrap());
        if let Ok(s) = used_rust_type(env, par.typ) {
            used_types.push(s);
        }
        bounds.add_for_parameter(env, func, par);
        let type_error = parameter_rust_type(env, par.typ, par.direction, Nullable(false), RefMode::None).is_err();
        if type_error {
            commented = true;
        }
    }

    let (outs, unsupported_outs) = out_parameters::analyze(env, func, configured_functions);
    if unsupported_outs {
        warn!("Function {} has unsupported outs", func.c_identifier.as_ref().unwrap_or(&func.name));
        commented = true;
    } else if !outs.is_empty() && !commented {
        out_parameters::analyze_imports(env, func, imports);
    }

    if !commented {
        imports.add_used_types(&used_types, version);
        if ret.base_tid.is_some() {
            imports.add("glib::object::Downcast", None);
        }
        bounds.update_imports(imports);
    }

    let visibility = if commented { Visibility::Comment } else { Visibility::Public };
    let assertion = SafetyAssertionMode::of(env, &parameters);

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
    }
}
