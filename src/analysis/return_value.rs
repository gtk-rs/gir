use log::error;

use crate::{
    analysis::{
        self, imports::Imports, namespaces, override_string_type::override_string_type_return,
        rust_type::RustType,
    },
    config,
    env::Env,
    library::{self, Nullable, TypeId},
};

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub parameter: Option<analysis::Parameter>,
    pub base_tid: Option<library::TypeId>, // Some only if need downcast
    pub commented: bool,
    pub bool_return_is_error: Option<String>,
    pub nullable_return_is_error: Option<String>,
}

pub fn analyze(
    env: &Env,
    obj: &config::gobjects::GObject,
    func: &library::Function,
    type_tid: library::TypeId,
    configured_functions: &[&config::functions::Function],
    used_types: &mut Vec<String>,
    imports: &mut Imports,
) -> Info {
    let typ = configured_functions
        .iter()
        .find_map(|f| f.ret.type_name.as_ref())
        .and_then(|typ| env.library.find_type(0, typ))
        .unwrap_or_else(|| override_string_type_return(env, func.ret.typ, configured_functions));
    let mut parameter = if typ == Default::default() {
        None
    } else {
        let mut nullable = func.ret.nullable;
        if !obj.trust_return_value_nullability {
            // Since GIRs are bad at specifying return value nullability, assume
            // any returned pointer is nullable unless overridden by the config.
            if !*nullable && can_be_nullable_return(env, typ) {
                *nullable = true;
            }
        }

        let nullable_override = configured_functions.iter().find_map(|f| f.ret.nullable);
        if let Some(val) = nullable_override {
            nullable = val;
        }
        Some(library::Parameter {
            typ,
            nullable,
            ..func.ret.clone()
        })
    };

    let mut commented = false;

    let bool_return_is_error = configured_functions
        .iter()
        .find_map(|f| f.ret.bool_return_is_error.as_ref());
    let bool_return_error_message = bool_return_is_error.and_then(|m| {
        if typ != TypeId::tid_bool() && typ != TypeId::tid_c_bool() {
            error!(
                "Ignoring bool_return_is_error configuration for non-bool returning function {}",
                func.name
            );
            None
        } else {
            let ns = if env.namespaces.glib_ns_id == namespaces::MAIN {
                "error"
            } else {
                "glib"
            };
            imports.add(ns);

            Some(m.clone())
        }
    });

    let nullable_return_is_error = configured_functions
        .iter()
        .find_map(|f| f.ret.nullable_return_is_error.as_ref());
    let nullable_return_error_message = nullable_return_is_error.and_then(|m| {
        if let Some(library::Parameter { nullable: Nullable(false), ..}) = parameter {
            error!(
                "Ignoring nullable_return_is_error configuration for non-none returning function {}",
                func.name
            );
            None
        } else {
            let ns = if env.namespaces.glib_ns_id == namespaces::MAIN {
                "crate::BoolError"
            } else {
                "glib"
            };
            imports.add(ns);

            Some(m.clone())
        }
    });

    let mut base_tid = None;

    if func.kind == library::FunctionKind::Constructor {
        if let Some(par) = parameter {
            let nullable_override = configured_functions.iter().find_map(|f| f.ret.nullable);
            if par.typ != type_tid {
                base_tid = Some(par.typ);
            }
            parameter = Some(library::Parameter {
                typ: type_tid,
                nullable: nullable_override.unwrap_or(func.ret.nullable),
                ..par
            });
        }
    }

    let parameter = parameter.as_ref().map(|lib_par| {
        let par = analysis::Parameter::from_return_value(env, lib_par, configured_functions);
        if let Ok(rust_type) = RustType::builder(env, typ)
            .direction(par.lib_par.direction)
            .try_from_glib(&par.try_from_glib)
            .try_build()
        {
            used_types.extend(rust_type.into_used_types());
        }

        commented = RustType::builder(env, typ)
            .direction(func.ret.direction)
            .try_from_glib(&par.try_from_glib)
            .try_build_param()
            .is_err();

        par
    });

    Info {
        parameter,
        base_tid,
        commented,
        bool_return_is_error: bool_return_error_message,
        nullable_return_is_error: nullable_return_error_message,
    }
}

fn can_be_nullable_return(env: &Env, type_id: library::TypeId) -> bool {
    use crate::library::{Basic::*, Type::*};
    match env.library.type_(type_id) {
        Basic(fund) => matches!(fund, Pointer | Utf8 | Filename | OsString),
        Alias(alias) => can_be_nullable_return(env, alias.typ),
        Enumeration(_) => false,
        Bitfield(_) => false,
        Record(_) => true,
        Union(_) => true,
        Function(_) => true,
        Interface(_) => true,
        Class(_) => true,
        _ => true,
    }
}
