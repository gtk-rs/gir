use crate::{config, env::Env, library::*};

pub fn override_string_type_parameter(
    env: &Env,
    typ: TypeId,
    configured_parameters: &[&config::functions::Parameter],
) -> TypeId {
    let string_type = configured_parameters
        .iter()
        .filter_map(|p| p.string_type)
        .next();
    apply(env, typ, string_type)
}

pub fn override_string_type_return(
    env: &Env,
    typ: TypeId,
    configured_functions: &[&config::functions::Function],
) -> TypeId {
    let string_type = configured_functions
        .iter()
        .filter_map(|f| f.ret.string_type)
        .next();
    apply(env, typ, string_type)
}

fn apply(env: &Env, type_id: TypeId, string_type: Option<config::StringType>) -> TypeId {
    use crate::library::*;
    let string_type = if let Some(string_type) = string_type {
        string_type
    } else {
        return type_id;
    };

    let replace = {
        use crate::config::StringType::*;
        match string_type {
            Utf8 => TypeId::tid_utf8(),
            Filename => TypeId::tid_filename(),
            OsString => TypeId::tid_os_string(),
        }
    };
    match *env.library.type_(type_id) {
        Type::Fundamental(Fundamental::Filename) => replace,
        Type::Fundamental(Fundamental::OsString) => replace,
        Type::Fundamental(Fundamental::Utf8) => replace,
        Type::CArray(inner_tid) if can_overriden_fundamental(env, inner_tid) => {
            Type::find_c_array(&env.library, replace, None)
        }
        _ => {
            error!(
                "Bad type {0} for string_type override",
                type_id.full_name(&env.library)
            );
            type_id
        }
    }
}

fn can_overriden_fundamental(env: &Env, type_id: TypeId) -> bool {
    use crate::library::*;
    match *env.library.type_(type_id) {
        Type::Fundamental(Fundamental::Filename) => true,
        Type::Fundamental(Fundamental::OsString) => true,
        Type::Fundamental(Fundamental::Utf8) => true,
        _ => false,
    }
}
