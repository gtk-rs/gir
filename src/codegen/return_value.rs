use std::cmp;

use crate::{
    analysis::{
        self, conversion_type::ConversionType, namespaces, out_parameters::Mode,
        rust_type::RustType, try_from_glib::TryFromGlib,
    },
    env::Env,
    library::{self, ParameterDirection, TypeId},
    nameutil::{is_gstring, mangle_keywords, use_glib_type},
    traits::*,
};

pub trait ToReturnValue {
    fn to_return_value(
        &self,
        env: &Env,
        try_from_glib: &TryFromGlib,
        is_trampoline: bool,
    ) -> Option<String>;
}

impl ToReturnValue for library::Parameter {
    fn to_return_value(
        &self,
        env: &Env,
        try_from_glib: &TryFromGlib,
        is_trampoline: bool,
    ) -> Option<String> {
        let mut name = RustType::builder(env, self.typ)
            .direction(self.direction)
            .nullable(self.nullable)
            .scope(self.scope)
            .try_from_glib(try_from_glib)
            .try_build_param()
            .into_string();
        if is_trampoline
            && self.direction == library::ParameterDirection::Return
            && is_gstring(&name)
        {
            name = "String".to_owned();
        }
        let type_str = match ConversionType::of(env, self.typ) {
            ConversionType::Unknown => format!("/*Unknown conversion*/{name}"),
            // TODO: records as in gtk_container_get_path_for_child
            _ => name,
        };

        Some(type_str)
    }
}

impl ToReturnValue for analysis::return_value::Info {
    fn to_return_value(
        &self,
        env: &Env,
        try_from_glib: &TryFromGlib,
        is_trampoline: bool,
    ) -> Option<String> {
        let par = self.parameter.as_ref()?;
        par.lib_par
            .to_return_value(env, try_from_glib, is_trampoline)
            .map(|type_name| {
                if self.nullable_return_is_error.is_some() && type_name.starts_with("Option<") {
                    // Change `Option<T>` to `Result<T, glib::BoolError>`
                    format!(
                        "Result<{}, {}BoolError>",
                        &type_name[7..(type_name.len() - 1)],
                        if env.namespaces.glib_ns_id == namespaces::MAIN {
                            ""
                        } else {
                            "glib::"
                        }
                    )
                } else {
                    type_name
                }
            })
    }
}

/// Returns the `TypeId` of the returned types from the provided function.
pub fn out_parameter_types(analysis: &analysis::functions::Info) -> Vec<TypeId> {
    // If it returns an error, there is nothing for us to check.
    if analysis.ret.bool_return_is_error.is_some()
        || analysis.ret.nullable_return_is_error.is_some()
    {
        return Vec::new();
    }

    if !analysis.outs.is_empty() {
        let num_out_args = analysis
            .outs
            .iter()
            .filter(|out| out.lib_par.array_length.is_none())
            .count();
        let num_out_sizes = analysis
            .outs
            .iter()
            .filter(|out| out.lib_par.array_length.is_some())
            .count();
        // We need to differentiate between array(s)'s size arguments and normal ones.
        // If we have 2 "normal" arguments and one "size" argument, we still
        // need to wrap them into "()" so we take that into account. If the
        // opposite, it means that there are two arguments in any case so
        // we need "()" too.
        let num_outs = std::cmp::max(num_out_args, num_out_sizes);
        match analysis.outs.mode {
            Mode::Normal | Mode::Combined => {
                let array_lengths: Vec<_> = analysis
                    .outs
                    .iter()
                    .filter_map(|out| out.lib_par.array_length)
                    .collect();
                let mut ret_params = Vec::with_capacity(num_outs);

                for out in analysis.outs.iter().filter(|out| !out.lib_par.is_error) {
                    // The actual return value is inserted with an empty name at position 0
                    if !out.lib_par.name.is_empty() {
                        let mangled_par_name =
                            crate::nameutil::mangle_keywords(out.lib_par.name.as_str());
                        let param_pos = analysis
                            .parameters
                            .c_parameters
                            .iter()
                            .enumerate()
                            .find_map(|(pos, orig_par)| {
                                if orig_par.name == mangled_par_name {
                                    Some(pos)
                                } else {
                                    None
                                }
                            })
                            .unwrap();
                        if array_lengths.contains(&(param_pos as u32)) {
                            continue;
                        }
                    }
                    ret_params.push(out.lib_par.typ);
                }
                ret_params
            }
            _ => Vec::new(),
        }
    } else if let Some(typ) = analysis.ret.parameter.as_ref().map(|out| out.lib_par.typ) {
        vec![typ]
    } else {
        Vec::new()
    }
}

fn out_parameter_as_return_parts(
    analysis: &analysis::functions::Info,
    env: &Env,
) -> (&'static str, String) {
    use crate::analysis::out_parameters::Mode::*;
    let num_out_args = analysis
        .outs
        .iter()
        .filter(|out| out.lib_par.array_length.is_none())
        .count();
    let num_out_sizes = analysis
        .outs
        .iter()
        .filter(|out| out.lib_par.array_length.is_some())
        .count();
    // We need to differentiate between array(s)'s size arguments and normal ones.
    // If we have 2 "normal" arguments and one "size" argument, we still need to
    // wrap them into "()" so we take that into account. If the opposite, it
    // means that there are two arguments in any case so we need "()" too.
    let num_outs = cmp::max(num_out_args, num_out_sizes);
    match analysis.outs.mode {
        Normal | Combined => {
            if num_outs > 1 {
                ("(", ")".to_owned())
            } else {
                ("", String::new())
            }
        }
        Optional => {
            if num_outs > 1 {
                if analysis.ret.nullable_return_is_error.is_some() {
                    (
                        "Result<(",
                        format!("), {}>", use_glib_type(env, "BoolError")),
                    )
                } else {
                    ("Option<(", ")>".to_owned())
                }
            } else if analysis.ret.nullable_return_is_error.is_some() {
                ("Result<", format!(", {}>", use_glib_type(env, "BoolError")))
            } else {
                ("Option<", ">".to_owned())
            }
        }
        Throws(..) => {
            if num_outs == 1 + 1 {
                // if only one parameter except "glib::Error"
                ("Result<", format!(", {}>", use_glib_type(env, "Error")))
            } else {
                ("Result<(", format!("), {}>", use_glib_type(env, "Error")))
            }
        }
        None => unreachable!(),
    }
}

pub fn out_parameters_as_return(env: &Env, analysis: &analysis::functions::Info) -> String {
    let (prefix, suffix) = out_parameter_as_return_parts(analysis, env);
    let mut return_str = String::with_capacity(100);
    return_str.push_str(" -> ");
    return_str.push_str(prefix);

    let array_lengths: Vec<_> = analysis
        .outs
        .iter()
        .filter_map(|out| out.lib_par.array_length)
        .collect();

    let mut skip = 0;
    for (pos, out) in analysis
        .outs
        .iter()
        .filter(|out| !out.lib_par.is_error)
        .enumerate()
    {
        // The actual return value is inserted with an empty name at position 0
        if !out.lib_par.name.is_empty() {
            let mangled_par_name = mangle_keywords(out.lib_par.name.as_str());
            let param_pos = analysis
                .parameters
                .c_parameters
                .iter()
                .enumerate()
                .find_map(|(pos, orig_par)| {
                    if orig_par.name == mangled_par_name {
                        Some(pos)
                    } else {
                        None
                    }
                })
                .unwrap();
            if array_lengths.contains(&(param_pos as u32)) {
                skip += 1;
                continue;
            }
        }

        if pos > skip {
            return_str.push_str(", ");
        }
        let s = out_parameter_as_return(out, env);
        return_str.push_str(&s);
    }
    return_str.push_str(&suffix);
    return_str
}

fn out_parameter_as_return(out: &analysis::Parameter, env: &Env) -> String {
    // TODO: upcasts?
    let name = RustType::builder(env, out.lib_par.typ)
        .direction(ParameterDirection::Return)
        .nullable(out.lib_par.nullable)
        .scope(out.lib_par.scope)
        .try_from_glib(&out.try_from_glib)
        .try_build_param()
        .into_string();
    match ConversionType::of(env, out.lib_par.typ) {
        ConversionType::Unknown => format!("/*Unknown conversion*/{name}"),
        _ => name,
    }
}
