use config;
use config::parameter_matchable::ParameterMatchable;
use env::Env;
use library;
use nameutil;
use super::conversion_type::ConversionType;
use super::ref_mode::RefMode;

#[derive(Clone, Debug)]
pub struct RustParameter {
    pub name: String,
    pub typ: library::TypeId,
    pub direction: library::ParameterDirection,
    pub nullable: library::Nullable,
    pub ref_mode: RefMode,
}

#[derive(Clone, Debug)]
pub struct CParameter {
    pub name: String,
    pub typ: library::TypeId,
    pub c_type: String,
}

#[derive(Clone, Debug)]
pub enum TransformationType {
    None,
}

#[derive(Clone, Debug)]
pub struct Transformation {
    pub ind_c: usize,     //index in `Vec<CParameter>`
    pub ind_rust: usize,  //index in `Vec<RustParameter>`
    pub transformation: TransformationType,
    pub name: String,
    pub typ: library::TypeId,
    pub transfer: library::Transfer,
    pub ref_mode: RefMode,
    pub conversion_type: ConversionType,
}

#[derive(Clone, Default, Debug)]
pub struct Parameters {
    pub rust_parameters: Vec<RustParameter>,
    pub c_parameters: Vec<CParameter>,
    pub transformations: Vec<Transformation>,
}

impl Parameters {
    fn new(capacity: usize) -> Parameters {
        Parameters {
            rust_parameters: Vec::with_capacity(capacity),
            c_parameters: Vec::with_capacity(capacity),
            transformations: Vec::with_capacity(capacity),
        }
    }

    //TODO: temp
    fn push(&mut self, type_tid: library::TypeId, name: String, c_type: String,
            direction: library::ParameterDirection, transfer: library::Transfer,
            nullable: library::Nullable, ref_mode: RefMode,
            conversion_type: ConversionType) {
        let c_par = CParameter {
            name: name.clone(),
            typ: type_tid,
            c_type: c_type,
        };
        let ind_c = self.c_parameters.len();
        self.c_parameters.push(c_par);

        let rust_par = RustParameter {
            name: name.clone(),
            typ: type_tid,
            direction: direction,
            nullable: nullable,
            ref_mode: ref_mode,
        };
        let ind_rust = self.rust_parameters.len();
        self.rust_parameters.push(rust_par);

        let transform = Transformation {
            ind_c: ind_c,
            ind_rust: ind_rust,
            transformation: TransformationType::None,
            name: name,
            typ: type_tid,
            transfer: transfer,
            ref_mode: ref_mode,
            conversion_type: conversion_type,
        };
        self.transformations.push(transform);
    }

    pub fn get(&self, ind_rust: usize) -> Option<&Transformation> {
        self.transformations.iter()
            .filter(|tr| tr.ind_rust==ind_rust)
            .next()
    }
}

pub fn analyze(env: &Env, signal_parameters: &[library::Parameter], type_tid: library::TypeId,
               configured_signals: &[&config::signals::Signal]) -> Parameters {
    let mut parameters = Parameters::new(signal_parameters.len() + 1);

    let owner = env.type_(type_tid);
    let c_type = format!("{}*", owner.get_glib_name().unwrap());

    parameters.push(type_tid, "this".to_owned(), c_type,
                    library::ParameterDirection::In, library::Transfer::None,
                    library::Nullable(false), RefMode::ByRef,
                    ConversionType::Pointer);

    for par in signal_parameters {
        let name = nameutil::mangle_keywords(&*par.name).into_owned();

        let ref_mode = RefMode::without_unneeded_mut(&env.library, par, false);

        let nullable_override = configured_signals.matched_parameters(&name).iter()
            .filter_map(|p| p.nullable)
            .next();
        let nullable = nullable_override.unwrap_or(par.nullable);

        let conversion_type = ConversionType::of(&env.library, par.typ);

        parameters.push(par.typ, name, par.c_type.clone(), par.direction,
                        par.transfer, nullable, ref_mode, conversion_type);
    }

    parameters
}
