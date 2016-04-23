use std::vec::Vec;

use analysis::bounds::Bounds;
use analysis::imports::Imports;
use analysis::out_parameters;
use analysis::parameter;
use analysis::ref_mode::RefMode;
use analysis::return_value;
use analysis::rust_type::*;
use analysis::safety_assertion_mode::SafetyAssertionMode;
use config;
use env::Env;
use library::{self, Nullable, ParameterDirection};
use nameutil;
use traits::*;
use version::Version;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Comment,
    Private,
    Hidden,
}

//TODO: change use Parameter to reference?
#[derive(Debug)]
pub struct Info<'e> {
    pub name: String,
    pub glib_name: String,
    pub kind: library::FunctionKind,
    pub visibility: Visibility,
    pub type_name: Result<'e>,
    pub parameters: Vec<parameter::Parameter<'e>>,
    pub ret: return_value::Info,
    pub bounds: Bounds,
    pub outs: out_parameters::Info,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
    pub cfg_condition: Option<String>,
    pub assertion: SafetyAssertionMode,
}

pub fn analyze<'e>(env: &'e Env, functions: &'e [library::Function], type_tid: library::TypeId,
               obj: &config::gobjects::GObject, imports: &mut Imports) -> Vec<Info<'e>> {
    let mut funcs = Vec::new();

    for func in functions {
        let configured_functions = obj.functions.matched(&func.name);
        if configured_functions.iter().any(|f| f.ignore) {
            continue;
        }
        let info = analyze_function(env, func, type_tid, &configured_functions, imports);
        funcs.push(info);
    }

    funcs
}

fn analyze_function<'e>(env: &'e Env, func: &'e library::Function, type_tid: library::TypeId,
                    configured_functions: &[&config::functions::Function],
                    imports: &mut Imports) -> Info<'e> {
    let mut commented = false;
    let mut bounds: Bounds = Default::default();
    let mut used_types: Vec<String> = Vec::with_capacity(4);

    let version = configured_functions.iter().filter_map(|f| f.version).min()
        .or(func.version);
    let version = env.config.filter_version(version);
    let deprecated_version = func.deprecated_version;
    let cfg_condition = configured_functions.iter().filter_map(|f| f.cfg_condition.clone()).next();

    let ret = return_value::analyze(env, func, type_tid, configured_functions, &mut used_types);
    commented |= ret.commented;

    let parameters: Vec<parameter::Parameter> =
        func.parameters.iter().map(|par| parameter::analyze(env, par, configured_functions)).collect();

    for (pos, par) in parameters.iter().enumerate() {
        assert!(!par.instance_parameter || pos == 0,
            "Wrong instance parameter in {}", func.c_identifier.as_ref().unwrap());
        if let Ok(s) = used_rust_type(env, par.typ) {
            used_types.push(s.into_owned());
        }
        let type_error = parameter_rust_type(env, par.typ, par.direction, Nullable(false), RefMode::None).is_err();
        if !par.instance_parameter && par.direction != ParameterDirection::Out {
            if let Some(bound_type) = Bounds::type_for(env, par.typ) {
                let type_name = bounds_rust_type(env, par.typ);
                if !bounds.add_parameter(&par.name, type_name.to_cow_str(), bound_type) {
                    panic!("Too many parameters upcasts for {}", func.c_identifier.as_ref().unwrap())
                }
            }
        }
        if type_error {
            commented = true;
        }
    }

    let (outs, unsupported_outs) = out_parameters::analyze(env, func);
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
        name: nameutil::mangle_keywords(&*func.name).into_owned(),
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
        cfg_condition: cfg_condition,
        assertion: assertion,
    }
}
