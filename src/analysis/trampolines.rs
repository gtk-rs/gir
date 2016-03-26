use config;
use env::Env;
use library;
use nameutil::signal_to_snake;
use super::bounds::{Bounds, BoundType};
use super::conversion_type::ConversionType;
use super::parameter;
use super::ref_mode::RefMode;
use super::rust_type::bounds_rust_type;
use traits::IntoString;

#[derive(Debug)]
pub struct Trampoline {
    pub name: String,
    pub parameters: Vec<parameter::Parameter>,
    pub ret: library::Parameter,
    pub bounds: Bounds,
}

pub type Trampolines = Vec<Trampoline>;

pub fn analyze(env: &Env, signal: &library::Signal, type_tid: library::TypeId, in_trait: bool,
               trampolines: &mut Trampolines) -> Option<String> {
    if !can_generate(env, signal) {
        return None;
    }

    let name = format!("{}_trampoline", signal_to_snake(&signal.name));

    let owner = env.type_(type_tid);

    //Fake
    let configured_functions: Vec<&config::functions::Function> = Vec::new();

    let mut bounds: Bounds = Default::default();

    let mut parameters: Vec<parameter::Parameter> = Vec::with_capacity(signal.parameters.len() + 1);

    let this = parameter::Parameter {
        name: "this".to_owned(),
        typ: type_tid,
        c_type: owner.get_glib_name().unwrap().to_owned(),
        instance_parameter: false, //true,
        direction: library::ParameterDirection::In,
        transfer: library::Transfer::None,
        caller_allocates: false,
        nullable: library::Nullable(false),
        allow_none: false,
        is_error: false,
        ref_mode: RefMode::ByRef,
        to_glib_extra: String::new(),
    };
    parameters.push(this);

    if in_trait {
        let type_name = bounds_rust_type(env, type_tid);
        bounds.add_parameter("this", &type_name.into_string(), BoundType::IsA);
    }

    for par in &signal.parameters {
        let analysis = parameter::analyze(env, par, &configured_functions);
        parameters.push(analysis);
    }

    let trampoline = Trampoline {
        name: name.clone(),
        parameters: parameters,
        ret: signal.ret.clone(),
        bounds: bounds,
    };
    trampolines.push(trampoline);
    Some(name)
}

fn can_generate(env: &Env, signal: &library::Signal) -> bool {
    if signal.ret.typ != Default::default() &&
        ConversionType::of(&env.library, signal.ret.typ) == ConversionType::Unknown {
            return false;
        }
    for par in &signal.parameters {
        if ConversionType::of(&env.library, par.typ) == ConversionType::Unknown {
            return false;
        }
    }
    true
}
