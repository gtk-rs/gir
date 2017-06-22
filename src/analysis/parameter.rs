use std::borrow::Cow;

use config::functions::Function;
use config::parameter_matchable::ParameterMatchable;
use env::Env;
use library;
use nameutil;
use super::conversion_type::ConversionType;
use super::ref_mode::RefMode;

#[derive(Clone, Debug)]
pub struct Parameter {
    //from library::Parameter
    pub name: String,
    pub typ: library::TypeId,
    pub c_type: String,
    pub instance_parameter: bool,
    pub direction: library::ParameterDirection,
    pub transfer: library::Transfer,
    pub caller_allocates: bool,
    pub nullable: library::Nullable,
    pub allow_none: bool,
    pub array_length: Option<u32>,
    pub is_error: bool,

    //analysis fields
    pub ref_mode: RefMode,
    //filled by functions
    //TODO: Find normal way to do it
    pub to_glib_extra: String,
    /// `true` if it is a type that can be put into an `Option`.
    pub is_into: bool,
}

fn is_into(env: &Env, par: &library::Parameter) -> bool {
    fn is_into_inner(env: &Env, par: &library::Type) -> bool {
        match *par {
            library::Type::Fundamental(fund) => {
                match fund {
                    library::Fundamental::Utf8 |
                    library::Fundamental::Type => true,
                    _ => false,
                }
            }
            library::Type::List(_) |
            library::Type::SList(_) |
            library::Type::CArray(_) => false,
            library::Type::Alias(ref alias) => is_into_inner(env, env.library.type_(alias.typ)),
            _ => true,
        }
    }
    !par.instance_parameter && is_into_inner(env, env.library.type_(par.typ))
}

pub fn analyze(
    env: &Env,
    par: &library::Parameter,
    configured_functions: &[&Function],
) -> Parameter {
    let name = if par.instance_parameter {
        Cow::Borrowed(&*par.name)
    } else {
        nameutil::mangle_keywords(&*par.name)
    };

    let immutable = configured_functions
        .matched_parameters(&name)
        .iter()
        .any(|p| p.constant);
    let ref_mode = RefMode::without_unneeded_mut(&env.library, par, immutable);

    let nullable_override = configured_functions
        .matched_parameters(&name)
        .iter()
        .filter_map(|p| p.nullable)
        .next();
    let nullable = nullable_override.unwrap_or(par.nullable);

    let mut caller_allocates = par.caller_allocates;
    let mut transfer = par.transfer;
    let conversion = ConversionType::of(&env.library, par.typ);
    if conversion == ConversionType::Direct || conversion == ConversionType::Scalar {
        //For simply types no reason to have these flags
        caller_allocates = false;
        transfer = library::Transfer::None;
    }

    Parameter {
        name: name.into_owned(),
        typ: par.typ,
        c_type: par.c_type.clone(),
        instance_parameter: par.instance_parameter,
        direction: par.direction,
        transfer: transfer,
        caller_allocates: caller_allocates,
        nullable: nullable,
        allow_none: par.allow_none,
        array_length: par.array_length,
        ref_mode: ref_mode,
        is_error: par.is_error,
        to_glib_extra: String::new(),
        is_into: *nullable && is_into(env, par),
    }
}
