use analysis::ref_mode::RefMode;
use analysis::rust_type::*;
use config;
use env::Env;
use library::{self, Nullable};

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub parameter: Option<library::Parameter>,
    pub base_tid: Option<library::TypeId>,  //Some only if need downcast
    pub commented: bool,
}

pub fn analyze(env: &Env, func: &library::Function, type_tid: library::TypeId,
               configured_functions: &[&config::functions::Function],
               used_types: &mut Vec<String>) -> Info {

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
        let nullable_override = configured_functions.iter()
            .filter_map(|f| f.ret.nullable)
            .next();
        if let Some(val) = nullable_override {
            nullable = val;
        }
        Some(library::Parameter {
                nullable: nullable,
                .. func.ret.clone()
            })
    };

    let commented = if func.ret.typ == Default::default() { false } else {
        parameter_rust_type(env, func.ret.typ, func.ret.direction, Nullable(false), RefMode::None).is_err()
    };

    let mut base_tid = None;

    if func.kind == library::FunctionKind::Constructor {
        if let Some(par) = parameter {
            if par.typ != type_tid {
                base_tid = Some(par.typ);
            }
            parameter = Some(library::Parameter {
                typ: type_tid,
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
    match *env.library.type_(type_id) {
        Fundamental(fund) => match fund {
            Pointer => true,
            Utf8 => true,
            Filename => true,
            _ => false,
        },
        Alias(ref alias) => can_be_nullable_return(env, alias.typ),
        Enumeration(_) => false,
        Bitfield(_) => false,
        Record(_) => true,
        Union(_) => true,
        Function(_) => true,
        Interface(_) => true,
        Class(_) => true,
        _ => true
    }
}
