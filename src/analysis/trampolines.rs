use config;
use env::Env;
use library;
use nameutil::signal_to_snake;
use parser::is_empty_c_type;
use super::bounds::{Bounds, BoundType};
use super::conversion_type::ConversionType;
use super::ffi_type::used_ffi_type;
use super::rust_type::{bounds_rust_type, rust_type, used_rust_type};
use super::trampoline_parameters::{self, Parameters};
use traits::IntoString;
use version::Version;

#[derive(Debug)]
pub struct Trampoline {
    pub name: String,
    pub parameters: Parameters,
    pub ret: library::Parameter,
    pub bounds: Bounds,
    pub version: Option<Version>,
    pub inhibit: bool,
}

pub type Trampolines = Vec<Trampoline>;

pub fn analyze(env: &Env, signal: &library::Signal, type_tid: library::TypeId, in_trait: bool,
               configured_signals: &[&config::signals::Signal],
               trampolines: &mut Trampolines, used_types: &mut Vec<String>,
               version: Option<Version>) -> Result<String, Vec<String>> {
    let errors = closure_errors(env, signal);
    if !errors.is_empty() {
        warn!("Can't generate {} trampoline for signal '{}'", type_tid.full_name(&env.library),
              signal.name);
        return Err(errors);
    }

    let name = format!("{}_trampoline", signal_to_snake(&signal.name));

    //TODO: move to object.signal.return config
    let inhibit = configured_signals.iter().any(|f| f.inhibit);
    if inhibit {
        if signal.ret.typ != library::TypeId::tid_bool() {
            error!("Wrong return type for Inhibit for signal '{}'", signal.name);
        }
        used_types.push("::signal::Inhibit".into());
    }

    let mut bounds: Bounds = Default::default();

    if in_trait {
        let type_name = bounds_rust_type(env, type_tid);
        bounds.add_parameter("this", &type_name.into_string(), BoundType::IsA,
                             library::Nullable(false));
    }

    let parameters = trampoline_parameters::analyze(env, &signal.parameters,
                                                    type_tid, configured_signals);

    if in_trait {
        let type_name = bounds_rust_type(env, type_tid);
        bounds.add_parameter("this", &type_name.into_string(), BoundType::IsA,
                             library::Nullable(false));
    }


    for par in &parameters.rust_parameters {
        if let Ok(s) = used_rust_type(env, par.typ) {
            used_types.push(s);
        }
    }
    for par in &parameters.c_parameters {
        if let Some(s) = used_ffi_type(env, par.typ, &par.c_type) {
            used_types.push(s);
        }
    }

    let mut ret_nullable = signal.ret.nullable;

    if signal.ret.typ != Default::default() {
        if let Ok(s) = used_rust_type(env, signal.ret.typ) {
            used_types.push(s);
        }
        if let Some(s) = used_ffi_type(env, signal.ret.typ, &signal.ret.c_type) {
            used_types.push(s);
        }

        let nullable_override = configured_signals.iter()
            .filter_map(|f| f.ret.nullable)
            .next();
        if let Some(nullable) = nullable_override {
            ret_nullable = nullable;
        }
    }

    let ret = library::Parameter {
        nullable: ret_nullable,
        .. signal.ret.clone()
    };

    let trampoline = Trampoline {
        name: name.clone(),
        parameters: parameters,
        ret: ret,
        bounds: bounds,
        version: version,
        inhibit: inhibit,
    };
    trampolines.push(trampoline);
    Ok(name)
}

fn closure_errors(env: &Env, signal: &library::Signal) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();
    for par in &signal.parameters {
        if let Some(error) = type_error(env, par) {
            errors.push(format!("{} {}: {}", error, par.name,
                                par.typ.full_name(&env.library)));
        }
    }
    if signal.ret.typ != Default::default() {
        if let Some(error) = type_error(env, &signal.ret) {
            errors.push(format!("{} return value {}", error,
                                signal.ret.typ.full_name(&env.library)));
        }
    }
    errors
}

fn type_error(env: &Env, par: &library::Parameter) -> Option<&'static str> {
    use super::rust_type::TypeError::*;
    if par.direction == library::ParameterDirection::Out {
        Some("Out")
    } else if par.direction == library::ParameterDirection::InOut {
        Some("InOut")
    } else if is_empty_c_type(&par.c_type) {
        Some("Empty ctype")
    } else if ConversionType::of(&env.library, par.typ) == ConversionType::Unknown {
        Some("Unknown conversion")
    } else {
        match rust_type(env, par.typ) {
            Err(Ignored(_)) => Some("Ignored"),
            Err(Mismatch(_)) => Some("Mismatch"),
            Err(Unimplemented(_)) => Some("Unimplemented"),
            Ok(_) => None,
        }
    }
}
