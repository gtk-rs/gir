use log::error;

use super::{
    bounds::{BoundType, Bounds},
    conversion_type::ConversionType,
    ffi_type::used_ffi_type,
    ref_mode::RefMode,
    rust_type::RustType,
    trampoline_parameters::{self, Parameters},
};
use crate::{
    config::{self, gobjects::GObject},
    env::Env,
    library,
    nameutil::signal_to_snake,
    parser::is_empty_c_type,
    traits::IntoString,
    version::Version,
};

#[derive(Debug, Clone)]
pub struct Trampoline {
    pub name: String,
    pub parameters: Parameters,
    pub ret: library::Parameter,
    // This field is used for user callbacks in `codegen::function_body_chunk` when generating
    // inner C functions. We need to have the bound name in order to create variables and also to
    // pass to the C function bounds (otherwise it won't compile because it doesn't know how to
    // infer the bounds).
    pub bound_name: String,
    pub bounds: Bounds,
    pub version: Option<Version>,
    pub inhibit: bool,
    pub concurrency: library::Concurrency,
    pub is_notify: bool,
    pub scope: library::ParameterScope,
    /// It's used to group callbacks
    pub user_data_index: usize,
    pub destroy_index: usize,
    pub nullable: library::Nullable,
    /// This field is used to give the type name when generating the "IsA<X>"
    /// part.
    pub type_name: String,
}

pub type Trampolines = Vec<Trampoline>;

pub fn analyze(
    env: &Env,
    signal: &library::Signal,
    type_tid: library::TypeId,
    in_trait: bool,
    fundamental_type: bool,
    configured_signals: &[&config::signals::Signal],
    obj: &GObject,
    used_types: &mut Vec<String>,
    version: Option<Version>,
) -> Result<Trampoline, Vec<String>> {
    let errors = closure_errors(env, signal);
    if !errors.is_empty() {
        warn_main!(
            type_tid,
            "Can't generate {} trampoline for signal '{}'",
            type_tid.full_name(&env.library),
            signal.name
        );
        return Err(errors);
    }

    let is_notify = signal.name.starts_with("notify::");

    let name = format!("{}_trampoline", signal_to_snake(&signal.name));

    // TODO: move to object.signal.return config
    let inhibit = configured_signals.iter().any(|f| f.inhibit);
    if inhibit && signal.ret.typ != library::TypeId::tid_bool() {
        error!("Wrong return type for Inhibit for signal '{}'", signal.name);
    }

    let mut bounds: Bounds = Default::default();

    if in_trait || fundamental_type {
        let type_name = RustType::builder(env, type_tid)
            .ref_mode(RefMode::ByRefFake)
            .try_build();
        if fundamental_type {
            bounds.add_parameter(
                "this",
                &type_name.into_string(),
                BoundType::AsRef(None),
                false,
            );
        } else {
            bounds.add_parameter(
                "this",
                &type_name.into_string(),
                BoundType::IsA(None),
                false,
            );
        }
    }

    let parameters = if is_notify {
        let mut parameters = trampoline_parameters::Parameters::new(1);

        let owner = env.type_(type_tid);
        let c_type = format!("{}*", owner.get_glib_name().unwrap());

        let transform = parameters.prepare_transformation(
            env,
            type_tid,
            "this".to_owned(),
            c_type,
            library::ParameterDirection::In,
            library::Transfer::None,
            library::Nullable(false),
            crate::analysis::ref_mode::RefMode::ByRef,
            ConversionType::Borrow,
        );
        parameters.transformations.push(transform);

        parameters
    } else {
        trampoline_parameters::analyze(env, &signal.parameters, type_tid, configured_signals, None)
    };

    if in_trait || fundamental_type {
        let type_name = RustType::builder(env, type_tid)
            .ref_mode(RefMode::ByRefFake)
            .try_build();
        if fundamental_type {
            bounds.add_parameter(
                "this",
                &type_name.into_string(),
                BoundType::AsRef(None),
                false,
            );
        } else {
            bounds.add_parameter(
                "this",
                &type_name.into_string(),
                BoundType::IsA(None),
                false,
            );
        }
    }

    for par in &parameters.rust_parameters {
        if let Ok(rust_type) = RustType::builder(env, par.typ)
            .direction(par.direction)
            .try_from_glib(&par.try_from_glib)
            .try_build()
        {
            used_types.extend(rust_type.into_used_types());
        }
    }
    for par in &parameters.c_parameters {
        if let Some(ffi_type) = used_ffi_type(env, par.typ, &par.c_type) {
            used_types.push(ffi_type);
        }
    }

    let mut ret_nullable = signal.ret.nullable;

    if signal.ret.typ != Default::default() {
        if let Ok(rust_type) = RustType::builder(env, signal.ret.typ)
            .direction(library::ParameterDirection::Out)
            .try_build()
        {
            // No GString
            used_types.extend(rust_type.into_used_types());
        }
        if let Some(ffi_type) = used_ffi_type(env, signal.ret.typ, &signal.ret.c_type) {
            used_types.push(ffi_type);
        }

        let nullable_override = configured_signals.iter().find_map(|f| f.ret.nullable);
        if let Some(nullable) = nullable_override {
            ret_nullable = nullable;
        }
    }

    let concurrency = configured_signals
        .iter()
        .map(|f| f.concurrency)
        .next()
        .unwrap_or(obj.concurrency);

    let ret = library::Parameter {
        nullable: ret_nullable,
        ..signal.ret.clone()
    };

    let trampoline = Trampoline {
        name,
        parameters,
        ret,
        bounds,
        version,
        inhibit,
        concurrency,
        is_notify,
        bound_name: String::new(),
        scope: library::ParameterScope::None,
        user_data_index: 0,
        destroy_index: 0,
        nullable: library::Nullable(false),
        type_name: env.library.type_(type_tid).get_name(),
    };
    Ok(trampoline)
}

fn closure_errors(env: &Env, signal: &library::Signal) -> Vec<String> {
    let mut errors: Vec<String> = Vec::new();
    for par in &signal.parameters {
        if let Some(error) = type_error(env, par) {
            errors.push(format!(
                "{} {}: {}",
                error,
                par.name,
                par.typ.full_name(&env.library)
            ));
        }
    }
    if signal.ret.typ != Default::default() {
        if let Some(error) = type_error(env, &signal.ret) {
            errors.push(format!(
                "{} return value {}",
                error,
                signal.ret.typ.full_name(&env.library)
            ));
        }
    }
    errors
}

pub fn type_error(env: &Env, par: &library::Parameter) -> Option<&'static str> {
    use super::rust_type::TypeError::*;
    if par.direction == library::ParameterDirection::Out {
        Some("Out")
    } else if par.direction == library::ParameterDirection::InOut {
        Some("InOut")
    } else if is_empty_c_type(&par.c_type) {
        Some("Empty ctype")
    } else if ConversionType::of(env, par.typ) == ConversionType::Unknown {
        Some("Unknown conversion")
    } else {
        match RustType::try_new(env, par.typ) {
            Err(Ignored(_)) => Some("Ignored"),
            Err(Mismatch(_)) => Some("Mismatch"),
            Err(Unimplemented(_)) => Some("Unimplemented"),
            Ok(_) => None,
        }
    }
}
