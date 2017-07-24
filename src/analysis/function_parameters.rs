use std::collections::HashMap;

use config;
use config::parameter_matchable::ParameterMatchable;
use env::Env;
use library;
use nameutil;
use super::conversion_type::ConversionType;
use super::rust_type::rust_type;
use super::ref_mode::RefMode;
use super::out_parameters::can_as_return;
use traits::IntoString;

//TODO: remove unused fields
#[derive(Clone, Debug)]
pub struct RustParameter {
    pub ind_c: usize, //index in `Vec<CParameter>`
    pub name: String,
    pub typ: library::TypeId,
    pub allow_none: bool,
}

#[derive(Clone, Debug)]
pub struct CParameter {
    pub name: String,
    pub typ: library::TypeId,
    pub c_type: String,
    pub instance_parameter: bool,
    pub direction: library::ParameterDirection,
    pub nullable: library::Nullable,
    pub transfer: library::Transfer,
    pub caller_allocates: bool,
    pub is_error: bool,

    //analysis fields
    pub ref_mode: RefMode,
    /// `true` if it is a type that can be put into an `Option`.
    pub is_into: bool,
}

#[derive(Clone, Debug)]
pub enum TransformationType {
    ToGlibDirect { name: String },
    ToGlibScalar {
        name: String,
        nullable: library::Nullable,
    },
    ToGlibPointer {
        name: String,
        instance_parameter: bool,
        transfer: library::Transfer,
        ref_mode: RefMode,
        //filled by functions
        to_glib_extra: String,
        is_into: bool,
    },
    ToGlibBorrow,
    ToGlibUnknown { name: String },
    Length {
        array_name: String,
        array_length_name: String,
        array_length_type: String,
    },
}

impl TransformationType {
    pub fn is_to_glib(&self) -> bool {
        use self::TransformationType::*;
        match *self {
            ToGlibDirect { .. } |
            ToGlibScalar { .. } |
            ToGlibPointer { .. } |
            ToGlibBorrow |
            ToGlibUnknown { .. } => true,
            _ => false,
        }
    }

    pub fn set_to_glib_extra(&mut self, to_glib_extra_: &str) {
        if let TransformationType::ToGlibPointer {
            ref mut to_glib_extra,
            ..
        } = *self
        {
            *to_glib_extra = to_glib_extra_.to_owned();
        }
    }
}

#[derive(Clone, Debug)]
pub struct Transformation {
    pub ind_c: usize,            //index in `Vec<CParameter>`
    pub ind_rust: Option<usize>, //index in `Vec<RustParameter>`
    pub transformation_type: TransformationType,
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

    pub fn analyze_return(&mut self, env: &Env, ret: &Option<library::Parameter>) {
        let array_length = if let Some(array_length) = ret.as_ref().and_then(|r| r.array_length) {
            array_length
        } else {
            return;
        };

        let ind_c = array_length as usize;

        let par = if let Some(par) = self.c_parameters.get(ind_c) {
            par
        } else {
            return;
        };

        let transformation = Transformation {
            ind_c: ind_c,
            ind_rust: None,
            transformation_type: get_length_type(env, "", &par.name, par.typ),
        };
        self.transformations.push(transformation);
    }
}

pub fn analyze(
    env: &Env,
    function_parameters: &[library::Parameter],
    configured_functions: &[&config::functions::Function],
) -> Parameters {
    let mut parameters = Parameters::new(function_parameters.len());

    //Map: length agrument position => array name
    let array_lengths: HashMap<u32, String> = function_parameters
        .iter()
        .filter_map(|p| p.array_length.map(|pos| (pos, p.name.clone())))
        .collect();

    for (pos, par) in function_parameters.iter().enumerate() {
        let name = if par.instance_parameter {
            par.name.clone()
        } else {
            nameutil::mangle_keywords(&*par.name).into_owned()
        };

        let ind_c = parameters.c_parameters.len();
        let mut ind_rust = Some(parameters.rust_parameters.len());

        let mut add_rust_parameter = match par.direction {
            library::ParameterDirection::In | library::ParameterDirection::InOut => true,
            library::ParameterDirection::Return => false,
            library::ParameterDirection::Out => !can_as_return(env, par),
        };

        if let Some(array_name) = array_lengths.get(&(pos as u32)) {
            add_rust_parameter = false;

            let transformation = Transformation {
                ind_c: ind_c,
                ind_rust: None,
                transformation_type: get_length_type(env, array_name, &par.name, par.typ),
            };
            parameters.transformations.push(transformation);
        }

        let mut caller_allocates = par.caller_allocates;
        let mut transfer = par.transfer;
        let conversion = ConversionType::of(&env.library, par.typ);
        if conversion == ConversionType::Direct || conversion == ConversionType::Scalar {
            //For simply types no reason to have these flags
            caller_allocates = false;
            transfer = library::Transfer::None;
        }

        let immutable = configured_functions
            .matched_parameters(&name)
            .iter()
            .any(|p| p.constant);
        let ref_mode = RefMode::without_unneeded_mut(env, par, immutable);

        let nullable_override = configured_functions
            .matched_parameters(&name)
            .iter()
            .filter_map(|p| p.nullable)
            .next();
        let nullable = nullable_override.unwrap_or(par.nullable);
        let is_into = *nullable && is_into(env, par);

        if add_rust_parameter {
            let rust_par = RustParameter {
                name: name.clone(),
                typ: par.typ,
                ind_c: ind_c,
                allow_none: par.allow_none,
            };
            parameters.rust_parameters.push(rust_par);
        } else {
            ind_rust = None;
        }

        let c_par = CParameter {
            name: name.clone(),
            typ: par.typ,
            c_type: par.c_type.clone(),
            instance_parameter: par.instance_parameter,
            direction: par.direction,
            transfer: transfer,
            caller_allocates: caller_allocates,
            nullable: nullable,
            ref_mode: ref_mode,
            is_error: par.is_error,
            is_into: is_into,
        };
        parameters.c_parameters.push(c_par);

        let transformation_type = match ConversionType::of(&env.library, par.typ) {
            ConversionType::Direct => TransformationType::ToGlibDirect { name: name },
            ConversionType::Scalar => TransformationType::ToGlibScalar {
                name: name,
                nullable: nullable,
            },
            ConversionType::Pointer => TransformationType::ToGlibPointer {
                name: name,
                instance_parameter: par.instance_parameter,
                transfer: transfer,
                ref_mode: ref_mode,
                to_glib_extra: String::new(),
                is_into: is_into,
            },
            ConversionType::Borrow => TransformationType::ToGlibBorrow,
            ConversionType::Unknown => TransformationType::ToGlibUnknown { name: name },
        };

        let transformation = Transformation {
            ind_c: ind_c,
            ind_rust: ind_rust,
            transformation_type: transformation_type,
        };
        parameters.transformations.push(transformation);
    }

    parameters
}

fn is_into(env: &Env, par: &library::Parameter) -> bool {
    fn is_into_inner(env: &Env, par: &library::Type) -> bool {
        match *par {
            library::Type::Fundamental(fund) => {
                match fund {
                    library::Fundamental::Utf8 | library::Fundamental::Type => true,
                    _ => false,
                }
            }
            library::Type::List(_) | library::Type::SList(_) | library::Type::CArray(_) => false,
            library::Type::Alias(ref alias) => is_into_inner(env, env.library.type_(alias.typ)),
            _ => true,
        }
    }
    !par.instance_parameter && is_into_inner(env, env.library.type_(par.typ))
}

fn get_length_type(
    env: &Env,
    array_name: &str,
    length_name: &str,
    length_typ: library::TypeId,
) -> TransformationType {
    let array_length_type = rust_type(env, length_typ).into_string();
    TransformationType::Length {
        array_name: array_name.to_string(),
        array_length_name: length_name.to_string(),
        array_length_type: array_length_type,
    }
}
