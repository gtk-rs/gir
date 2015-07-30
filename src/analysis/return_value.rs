use std::collections::HashSet;

use analysis::c_type::rustify_pointers;
use analysis::rust_type::*;
use env::Env;
use library;

pub struct Info {
    pub parameter: Option<library::Parameter>,
    pub base_tid: Option<library::TypeId>,  //Some only if need downcast
    pub commented: bool,
}

pub fn analyze(env: &Env, type_: &library::Function, class_tid: library::TypeId,
    used_types: &mut HashSet<String>) -> Info {

    let mut parameter = if type_.ret.typ == Default::default() { None } else {
        used_rust_type(env, type_.ret.typ).ok().map(|s| used_types.insert(s));
        Some(library::Parameter {
                //Many missing return nullables in girs so detecting it
                nullable: type_.ret.nullable ||
                    can_be_nullable_return(env, type_.ret.typ),
                ..type_.ret.clone()
            })
    };

    let commented = if type_.ret.typ == Default::default() { false } else {
        parameter_rust_type(env, type_.ret.typ, type_.ret.direction).is_err()
    };

    if type_.kind == library::FunctionKind::Constructor {
        if let Some(par) = parameter {
            parameter = Some(library::Parameter {
                typ: class_tid,
                nullable: false,
                ..par
            });
        }
    }

    let base_tid = if let Some(ref par) = parameter {
        get_base_type_id_from_c_type(env, par)
    } else {
        None
    };

    Info {
        parameter: parameter,
        base_tid: base_tid,
        commented: commented,
    }
}

fn can_be_nullable_return(env: &Env, type_id: library::TypeId) -> bool
{
    use library::Type::*;
    use library::Fundamental::*;
    match env.library.type_(type_id) {
        &Fundamental(fund) => match fund {
            Pointer => true,
            Utf8 => true,
            Filename => true,
            _ => false,
        },
        &Alias(ref alias) => can_be_nullable_return(env, alias.typ),
        &Enumeration(_) => false,
        &Bitfield(_) => false,
        &Record(_) => false,
        &Union(_) => false,
        &Function(_) => true,
        &Interface(_) => true,
        &Class(_) => true,
        _ => true
    }
}

fn get_base_type_id_from_c_type(env: &Env, par: &library::Parameter) -> Option<library::TypeId> {
    match env.type_(par.typ) {
        &library::Type::Class(_) => {
            let (_, inner) = rustify_pointers(&par.c_type);
            let typ = env.library.find_type_by_glib_name(&inner);
            match typ {
                Some(tid) => if tid == par.typ { None } else { typ },
                None => typ
            }
        }
        _ => None,
    }
}
