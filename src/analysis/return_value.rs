use analysis::rust_type::*;
use env::Env;
use library::{self, Nullable};

pub struct Info {
    pub parameter: Option<library::Parameter>,
    pub base_tid: Option<library::TypeId>,  //Some only if need downcast
    pub commented: bool,
}

pub fn analyze(env: &Env, func: &library::Function, class_tid: library::TypeId,
    non_nullable_overrides: &[String], used_types: &mut Vec<String>) -> Info {

    let mut parameter = if func.ret.typ == Default::default() { None } else {
        if let Ok(s) = used_rust_type(env, func.ret.typ) {
            used_types.push(s);
        }
        // Since GIRs are bad at specifying return value nullability, assume
        // any returned pointer is nullable unless overridden by the config.
        let mut nullable = func.ret.nullable;
        if !*nullable && can_be_nullable_return(env, func.ret.typ) {
            *nullable = true;
        }
        if *nullable && non_nullable_overrides.binary_search(&func.name).is_ok() {
            *nullable = false;
        }
        Some(library::Parameter {
                nullable: nullable,
                .. func.ret.clone()
            })
    };

    let commented = if func.ret.typ == Default::default() { false } else {
        parameter_rust_type(env, func.ret.typ, func.ret.direction, Nullable(false)).is_err()
    };

    let mut base_tid = None;

    if func.kind == library::FunctionKind::Constructor {
        if let Some(par) = parameter {
            if par.typ != class_tid {
                base_tid = Some(par.typ);
            }
            parameter = Some(library::Parameter {
                typ: class_tid,
                nullable: Nullable(false),
                ..par
            });
        }
    }

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
