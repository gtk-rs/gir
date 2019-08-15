use crate::{
    analysis::{
        conversion_type::ConversionType, function_parameters::CParameter,
        functions::is_carray_with_direct_elements, imports::Imports, ref_mode::RefMode,
        return_value, rust_type::parameter_rust_type,
    },
    config,
    env::Env,
    library::*,
    nameutil,
};
use std::slice::Iter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    None,
    Normal,
    Optional,
    Combined,
    //<use function return>
    Throws(bool),
}

impl Default for Mode {
    fn default() -> Mode {
        Mode::None
    }
}

#[derive(Debug, Default)]
pub struct Info {
    pub mode: Mode,
    pub params: Vec<Parameter>,
}

impl Info {
    pub fn is_empty(&self) -> bool {
        self.mode == Mode::None
    }

    pub fn iter(&self) -> Iter<'_, Parameter> {
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

    let nullable_override = configured_functions
        .iter()
        .filter_map(|f| f.ret.nullable)
        .next();
    if func.throws {
        let use_ret = use_return_value_for_result(env, func_ret);
        info.mode = Mode::Throws(use_ret);
    } else if func.ret.typ == TypeId::tid_none() {
        info.mode = Mode::Normal;
    } else if func.ret.typ == TypeId::tid_bool() {
        if nullable_override == Some(Nullable(false)) {
            info.mode = Mode::Combined;
        } else {
            info.mode = Mode::Optional;
        }
    } else {
        info.mode = Mode::Combined;
    }

    for par in &func.parameters {
        if par.direction != ParameterDirection::Out {
            continue;
        }
        if can_as_return(env, par) {
            let mut par = par.clone();
            par.name = nameutil::mangle_keywords(&*par.name).into_owned();
            //TODO: temporary solution for string_type override
            if let Some(c_par) = func_c_params.iter().find(|c_par| c_par.name == par.name) {
                par.typ = c_par.typ;
            }
            info.params.push(par);
        } else {
            unsupported_outs = true;
        }
    }

    if info.params.is_empty() {
        info.mode = Mode::None;
    }
    if info.mode == Mode::Combined || info.mode == Mode::Throws(true) {
        let mut ret = func.ret.clone();
        //TODO: fully switch to use analyzed returns (it add too many Return<Option<>>)
        if let Some(ref par) = func_ret.parameter {
            ret.typ = par.typ;
        }
        if let Some(val) = nullable_override {
            ret.nullable = val;
        }
        info.params.insert(0, ret);
    }

    (info, unsupported_outs)
}

pub fn analyze_imports(env: &Env, parameters: &[Parameter], imports: &mut Imports) {
    for par in parameters {
        if par.direction == ParameterDirection::Out {
            match *env.library.type_(par.typ) {
                Type::Bitfield(..) | Type::Enumeration(..) => imports.add("std::mem"),
                Type::Fundamental(fund)
                    if fund != Fundamental::Utf8
                        && fund != Fundamental::OsString
                        && fund != Fundamental::Filename =>
                {
                    imports.add("std::mem")
                }
                _ if !par.caller_allocates => match ConversionType::of(env, par.typ) {
                    ConversionType::Direct | ConversionType::Scalar => (),
                    _ => imports.add("std::ptr"),
                },
                _ => (),
            }
        }
    }
}

pub fn can_as_return(env: &Env, par: &Parameter) -> bool {
    use super::conversion_type::ConversionType::*;
    match ConversionType::of(env, par.typ) {
        Direct => true,
        Scalar => true,
        Pointer => {
            // Disallow fundamental arrays without length
            if is_carray_with_direct_elements(env, par.typ) && par.array_length.is_none() {
                return false;
            }

            parameter_rust_type(
                env,
                par.typ,
                ParameterDirection::Out,
                Nullable(false),
                RefMode::None,
                par.scope,
            )
            .is_ok()
        }
        Borrow => false,
        Unknown => false,
    }
}

pub fn use_return_value_for_result(env: &Env, ret: &return_value::Info) -> bool {
    if let Some(ref par) = ret.parameter {
        use_function_return_for_result(env, par.typ)
    } else {
        false
    }
}

pub fn use_function_return_for_result(env: &Env, typ: TypeId) -> bool {
    if typ == Default::default() {
        return false;
    }
    if typ.ns_id != INTERNAL_NAMESPACE {
        return true;
    }
    let type_ = env.type_(typ);
    match &*type_.get_name() {
        "UInt" => false,
        "Boolean" => false,
        _ => true,
    }
}
