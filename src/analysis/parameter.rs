use std::borrow::Cow;

use env::Env;
use library;
use nameutil;
use super::ref_mode::RefMode;

#[derive(Clone)]
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

    //analysis fields
    pub ref_mode: RefMode,
}

pub fn analyze(env: &Env, par: &library::Parameter) -> Parameter {
    let name = if par.instance_parameter {
        Cow::Borrowed(&*par.name)
    } else {
        nameutil::mangle_keywords(&*par.name)
    };

    let ref_mode = RefMode::without_unneeded_mut(&env.library, par);

    Parameter {
        name: name.into_owned(),
        typ: par.typ,
        c_type: par.c_type.clone(),
        instance_parameter: par.instance_parameter,
        direction: par.direction,
        transfer: par.transfer,
        caller_allocates: par.caller_allocates,
        nullable: par.nullable,
        allow_none: par.allow_none,
        ref_mode: ref_mode,
    }
}
