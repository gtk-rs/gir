use std::slice::Iter;

use log::error;

use crate::{
    analysis::{
        self, conversion_type::ConversionType, function_parameters::CParameter,
        functions::is_carray_with_direct_elements, imports::Imports, return_value,
        rust_type::RustType,
    },
    config::{self, parameter_matchable::ParameterMatchable},
    env::Env,
    library::{
        self, Basic, Function, Nullable, ParameterDirection, Type, TypeId, INTERNAL_NAMESPACE,
    },
    nameutil,
};

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThrowFunctionReturnStrategy {
    #[default]
    ReturnResult,
    CheckError,
    Void,
}

#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    #[default]
    None,
    Normal,
    Optional,
    Combined,
    Throws(ThrowFunctionReturnStrategy),
}

#[derive(Debug, Default)]
pub struct Info {
    pub mode: Mode,
    pub params: Vec<analysis::Parameter>,
}

impl Info {
    pub fn is_empty(&self) -> bool {
        self.mode == Mode::None
    }

    pub fn iter(&self) -> Iter<'_, analysis::Parameter> {
        self.params.iter()
    }
}

pub fn analyze(
    env: &Env,
    func: &Function,
    func_c_params: &[CParameter],
    func_ret: &return_value::Info,
    configured_functions: &[&config::functions::Function],
) -> (Info, bool) {
    let mut info: Info = Default::default();
    let mut unsupported_outs = false;

    let nullable_override = configured_functions.iter().find_map(|f| f.ret.nullable);
    if func.throws {
        let return_strategy =
            decide_throw_function_return_strategy(env, func_ret, &func.name, configured_functions);
        info.mode = Mode::Throws(return_strategy);
    } else if func.ret.typ == TypeId::tid_none() {
        info.mode = Mode::Normal;
    } else if func.ret.typ == TypeId::tid_bool() || func.ret.typ == TypeId::tid_c_bool() {
        if nullable_override == Some(Nullable(false)) {
            info.mode = Mode::Combined;
        } else {
            info.mode = Mode::Optional;
        }
    } else {
        info.mode = Mode::Combined;
    }

    for lib_par in &func.parameters {
        if lib_par.direction != ParameterDirection::Out {
            continue;
        }
        if can_as_return(env, lib_par) {
            let mut lib_par = lib_par.clone();
            lib_par.name = nameutil::mangle_keywords(&lib_par.name).into_owned();
            let configured_parameters = configured_functions.matched_parameters(&lib_par.name);
            let mut out =
                analysis::Parameter::from_parameter(env, &lib_par, &configured_parameters);

            // FIXME: temporary solution for string_type, nullable override. This should
            // completely work based on the analyzed parameters instead of the
            // library parameters.
            if let Some(c_par) = func_c_params
                .iter()
                .find(|c_par| c_par.name == lib_par.name)
            {
                out.lib_par.typ = c_par.typ;
                out.lib_par.nullable = c_par.nullable;
            }

            info.params.push(out);
        } else {
            unsupported_outs = true;
        }
    }

    if info.params.is_empty() {
        info.mode = Mode::None;
    }
    if info.mode == Mode::Combined
        || info.mode == Mode::Throws(ThrowFunctionReturnStrategy::ReturnResult)
    {
        let mut ret = analysis::Parameter::from_return_value(env, &func.ret, configured_functions);

        // TODO: fully switch to use analyzed returns (it add too many Return<Option<>>)
        if let Some(ref par) = func_ret.parameter {
            ret.lib_par.typ = par.lib_par.typ;
        }
        if let Some(val) = nullable_override {
            ret.lib_par.nullable = val;
        }
        info.params.insert(0, ret);
    }

    (info, unsupported_outs)
}

pub fn analyze_imports<'a>(
    env: &Env,
    parameters: impl IntoIterator<Item = &'a library::Parameter>,
    imports: &mut Imports,
) {
    for par in parameters {
        if par.direction == ParameterDirection::Out {
            analyze_type_imports(env, par.typ, par.caller_allocates, imports);
        }
    }
}

fn analyze_type_imports(env: &Env, typ: TypeId, caller_allocates: bool, imports: &mut Imports) {
    match env.library.type_(typ) {
        Type::Alias(alias) => analyze_type_imports(env, alias.typ, caller_allocates, imports),
        Type::Bitfield(..) | Type::Enumeration(..) => imports.add("std::mem"),
        Type::Basic(fund) if !matches!(fund, Basic::Utf8 | Basic::OsString | Basic::Filename) => {
            imports.add("std::mem");
        }
        _ if !caller_allocates => match ConversionType::of(env, typ) {
            ConversionType::Direct
            | ConversionType::Scalar
            | ConversionType::Option
            | ConversionType::Result { .. } => (),
            _ => imports.add("std::ptr"),
        },
        _ => (),
    }
}

pub fn can_as_return(env: &Env, par: &library::Parameter) -> bool {
    use super::conversion_type::ConversionType::*;
    match ConversionType::of(env, par.typ) {
        Direct | Scalar | Option | Result { .. } => true,
        Pointer => {
            // Disallow Basic arrays without length
            if is_carray_with_direct_elements(env, par.typ) && par.array_length.is_none() {
                return false;
            }

            RustType::builder(env, par.typ)
                .direction(ParameterDirection::Out)
                .scope(par.scope)
                .try_build_param()
                .is_ok()
        }
        Borrow => false,
        Unknown => false,
    }
}

fn decide_throw_function_return_strategy(
    env: &Env,
    ret: &return_value::Info,
    func_name: &str,
    configured_functions: &[&config::functions::Function],
) -> ThrowFunctionReturnStrategy {
    let typ = ret
        .parameter
        .as_ref()
        .map(|par| par.lib_par.typ)
        .unwrap_or_default();
    if env.type_(typ).eq(&Type::Basic(Basic::None)) {
        ThrowFunctionReturnStrategy::Void
    } else if use_function_return_for_result(env, typ, func_name, configured_functions) {
        ThrowFunctionReturnStrategy::ReturnResult
    } else {
        ThrowFunctionReturnStrategy::CheckError
    }
}

pub fn use_function_return_for_result(
    env: &Env,
    typ: TypeId,
    func_name: &str,
    configured_functions: &[&config::functions::Function],
) -> bool {
    // Configuration takes precendence over everything.
    let use_return_for_result = configured_functions
        .iter()
        .find_map(|f| f.ret.use_return_for_result.as_ref());
    if let Some(use_return_for_result) = use_return_for_result {
        if typ == Default::default() {
            error!("Function \"{}\": use_return_for_result set to true, but function has no return value", func_name);
            return false;
        }
        return *use_return_for_result;
    }

    if typ == Default::default() {
        return false;
    }
    if typ.ns_id != INTERNAL_NAMESPACE {
        return true;
    }
    let type_ = env.type_(typ);
    !matches!(&*type_.get_name(), "UInt" | "Boolean" | "Bool")
}
