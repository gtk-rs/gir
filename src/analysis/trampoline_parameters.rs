use super::{conversion_type::ConversionType, ref_mode::RefMode};
use crate::{
    analysis::is_gpointer,
    analysis::rust_type::rust_type,
    config::{self, parameter_matchable::ParameterMatchable},
    env::Env,
    library, nameutil,
};
use log::error;

pub use crate::config::signals::TransformationType;

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

impl CParameter {
    pub fn is_real_gpointer(&self, env: &Env) -> bool {
        is_gpointer(&self.c_type) && rust_type(env, self.typ).is_err()
    }
}

#[derive(Clone, Debug)]
pub struct Transformation {
    pub ind_c: usize,    //index in `Vec<CParameter>`
    pub ind_rust: usize, //index in `Vec<RustParameter>`
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
    pub fn new(capacity: usize) -> Parameters {
        Parameters {
            rust_parameters: Vec::with_capacity(capacity),
            c_parameters: Vec::with_capacity(capacity),
            transformations: Vec::with_capacity(capacity),
        }
    }

    pub fn prepare_transformation(
        &mut self,
        type_tid: library::TypeId,
        name: String,
        c_type: String,
        direction: library::ParameterDirection,
        transfer: library::Transfer,
        nullable: library::Nullable,
        ref_mode: RefMode,
        conversion_type: ConversionType,
    ) -> Transformation {
        let c_par = CParameter {
            name: name.clone(),
            typ: type_tid,
            c_type,
        };
        let ind_c = self.c_parameters.len();
        self.c_parameters.push(c_par);

        let rust_par = RustParameter {
            name: name.clone(),
            typ: type_tid,
            direction,
            nullable,
            ref_mode,
        };
        let ind_rust = self.rust_parameters.len();
        self.rust_parameters.push(rust_par);

        Transformation {
            ind_c,
            ind_rust,
            transformation: TransformationType::None,
            name,
            typ: type_tid,
            transfer,
            ref_mode,
            conversion_type,
        }
    }

    pub fn get(&self, ind_rust: usize) -> Option<&Transformation> {
        self.transformations
            .iter()
            .find(|tr| tr.ind_rust == ind_rust)
    }
}

pub fn analyze(
    env: &Env,
    signal_parameters: &[library::Parameter],
    type_tid: library::TypeId,
    configured_signals: &[&config::signals::Signal],
) -> Parameters {
    let mut parameters = Parameters::new(signal_parameters.len() + 1);

    let owner = env.type_(type_tid);
    let c_type = format!("{}*", owner.get_glib_name().unwrap());

    let transform = parameters.prepare_transformation(
        type_tid,
        "this".to_owned(),
        c_type,
        library::ParameterDirection::In,
        library::Transfer::None,
        library::Nullable(false),
        RefMode::ByRef,
        ConversionType::Borrow,
    );
    parameters.transformations.push(transform);

    for par in signal_parameters {
        let name = nameutil::mangle_keywords(&*par.name).into_owned();

        let ref_mode = RefMode::without_unneeded_mut(env, par, false, false);

        let nullable_override = configured_signals
            .matched_parameters(&name)
            .iter()
            .filter_map(|p| p.nullable)
            .next();
        let nullable = nullable_override.unwrap_or(par.nullable);

        let conversion_type = {
            match *env.library.type_(par.typ) {
                library::Type::Fundamental(library::Fundamental::Utf8)
                | library::Type::Record(..)
                | library::Type::Interface(..)
                | library::Type::Class(..) => ConversionType::Borrow,
                _ => ConversionType::of(env, par.typ),
            }
        };

        let new_name = configured_signals
            .matched_parameters(&name)
            .iter()
            .filter_map(|p| p.new_name.clone())
            .next();
        let transformation_override = configured_signals
            .matched_parameters(&name)
            .iter()
            .filter_map(|p| p.transformation)
            .next();

        let mut transform = parameters.prepare_transformation(
            par.typ,
            name,
            par.c_type.clone(),
            par.direction,
            par.transfer,
            nullable,
            ref_mode,
            conversion_type,
        );

        if let Some(new_name) = new_name {
            transform.name = new_name;
        }

        if let Some(transformation_type) = transformation_override {
            apply_transformation_type(env, &mut parameters, &mut transform, transformation_type);
        }
        parameters.transformations.push(transform);
    }

    parameters
}

fn apply_transformation_type(
    env: &Env,
    parameters: &mut Parameters,
    transform: &mut Transformation,
    transformation_type: TransformationType,
) {
    transform.transformation = transformation_type;
    match transformation_type {
        TransformationType::None => (),
        TransformationType::Borrow => {
            if transform.conversion_type == ConversionType::Pointer {
                transform.conversion_type = ConversionType::Borrow;
            } else if transform.conversion_type != ConversionType::Borrow {
                error!(
                    "Wrong conversion_type for borrow transformation {:?}",
                    transform.conversion_type
                );
            }
        }
        TransformationType::TreePath => {
            let type_ = env.type_(transform.typ);
            if let library::Type::Fundamental(library::Fundamental::Utf8) = *type_ {
                if let Some(type_tid) = env.library.find_type(0, "Gtk.TreePath") {
                    transform.typ = type_tid;
                    transform.conversion_type = ConversionType::Direct;
                    if let Some(rust_par) = parameters.rust_parameters.get_mut(transform.ind_rust) {
                        rust_par.typ = type_tid;
                        rust_par.ref_mode = RefMode::None;
                    }
                } else {
                    error!("Type Gtk.TreePath not found for treepath transformation");
                }
            } else {
                error!(
                    "Wrong parameter type for treepath transformation {:?}",
                    transform.typ
                );
            }
        }
    }
}
