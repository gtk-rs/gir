use std::borrow::Cow;

use env::Env;
use library;
use nameutil;

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
    pub by_ref: bool,
}

pub fn analyze(env: &Env, par: &library::Parameter) -> Parameter {
    let name = if par.instance_parameter {
        Cow::Borrowed(&*par.name)
    } else {
        nameutil::mangle_keywords(&*par.name)
    };

    let by_ref = use_by_ref(&env.library, par.typ, par.direction);

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
        by_ref: by_ref,
    }
}

#[inline]
pub fn use_by_ref(library: &library::Library, tid: library::TypeId, direction: library::ParameterDirection) -> bool {
    use library::Type::*;
    match *library.type_(tid) {
        Fundamental(library::Fundamental::Utf8) |
            Fundamental(library::Fundamental::Filename) |
            Record(..) |
            Class(..) |
            Interface(..) |
            List(..) => direction == library::ParameterDirection::In,
        Alias(ref alias) => use_by_ref(library, alias.typ, direction),
        _ => false,
    }
}
