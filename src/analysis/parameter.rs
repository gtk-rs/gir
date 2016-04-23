use std::borrow::Cow;

use config::functions::Function;
use env::Env;
use library;
use nameutil;
use super::bounds::Bounds;
use super::ref_mode::RefMode;

#[derive(Clone, Debug)]
pub struct Parameter<'e> {
    //from library::Parameter
    pub name: String,
    pub typ: library::TypeId,
    pub c_type: Cow<'e, str>,
    pub instance_parameter: bool,
    pub direction: library::ParameterDirection,
    pub transfer: library::Transfer,
    pub caller_allocates: bool,
    pub nullable: library::Nullable,
    pub allow_none: bool,
    pub is_error: bool,

    //analysis fields
    pub ref_mode: RefMode,
    //for AsRef trait bound
    //TODO: Find normal way to do it
    pub to_glib_extra: String,
}

pub fn analyze<'e>(env: &'e Env, par: &'e library::Parameter, configured_functions: &[&Function]) -> Parameter<'e> {
    let name = if par.instance_parameter {
        Cow::Borrowed(&*par.name)
    } else {
        nameutil::mangle_keywords(&*par.name)
    };

    let immutable = Function::matched_parameters(configured_functions, &name)
        .iter().any(|p| p.constant);
    let ref_mode = RefMode::without_unneeded_mut(&env.library, par, immutable);

    let nullable_override = Function::matched_parameters(configured_functions, &name).iter()
        .filter_map(|p| p.nullable)
        .next();
    let nullable = nullable_override.unwrap_or(par.nullable);
    let to_glib_extra = Bounds::to_glib_extra(&env.library, par.typ);

    Parameter {
        name: name.into_owned(),
        typ: par.typ,
        c_type: Cow::Borrowed(&*par.c_type),
        instance_parameter: par.instance_parameter,
        direction: par.direction,
        transfer: par.transfer,
        caller_allocates: par.caller_allocates,
        nullable: nullable,
        allow_none: par.allow_none,
        ref_mode: ref_mode,
        is_error: par.is_error,
        to_glib_extra: to_glib_extra,
    }
}
