use crate::{
    analysis::{
        self, conversion_type::ConversionType, namespaces, ref_mode::RefMode,
        rust_type::parameter_rust_type,
    },
    env::Env,
    library::{self, ParameterDirection},
    nameutil::{is_gstring, mangle_keywords, use_glib_type},
    traits::*,
};
use std::cmp;

pub trait ToReturnValue {
    fn to_return_value(&self, env: &Env, is_trampoline: bool) -> String;
}

impl ToReturnValue for library::Parameter {
    fn to_return_value(&self, env: &Env, is_trampoline: bool) -> String {
        let rust_type = parameter_rust_type(
            env,
            self.typ,
            self.direction,
            self.nullable,
            RefMode::None,
            self.scope,
        );
        let mut name = rust_type.into_string();
        if is_trampoline
            && self.direction == library::ParameterDirection::Return
            && is_gstring(&name)
        {
            name = "String".to_owned();
        }
        let type_str = match ConversionType::of(env, self.typ) {
            ConversionType::Unknown => format!("/*Unknown conversion*/{}", name),
            //TODO: records as in gtk_container_get_path_for_child
            _ => name,
        };
        format!(" -> {}", type_str)
    }
}

impl ToReturnValue for analysis::return_value::Info {
    fn to_return_value(&self, env: &Env, is_trampoline: bool) -> String {
        match self.parameter {
            Some(ref par) => {
                let name = par.to_return_value(env, is_trampoline);
                if self.nullable_return_is_error.is_some() && name.starts_with(" -> Option<") {
                    // Change ` -> Option<T>` to ` -> Result<T, glib::BoolError>`
                    format!(
                        " -> Result<{}, {}BoolError>",
                        &name[11..(name.len() - 1)],
                        if env.namespaces.glib_ns_id == namespaces::MAIN {
                            ""
                        } else {
                            "glib::"
                        }
                    )
                } else {
                    name
                }
            }
            None => String::new(),
        }
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
        .filter(|p| p.array_length.is_none())
        .count();
    let num_out_sizes = analysis
        .outs
        .iter()
        .filter(|p| p.array_length.is_some())
        .count();
    // We need to differentiate between array(s)'s size arguments and normal ones. If we have 2
    // "normal" arguments and one "size" argument, we still need to wrap them into "()" so we take
    // that into account. If the opposite, it means that there are two arguments in any case so
    // we need "()" too.
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
                //if only one parameter except "glib::Error"
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
        .filter_map(|p| p.array_length)
        .collect();

    let mut skip = 0;
    for (pos, par) in analysis.outs.iter().filter(|par| !par.is_error).enumerate() {
        // The actual return value is inserted with an empty name at position 0
        if !par.name.is_empty() {
            let mangled_par_name = mangle_keywords(par.name.as_str());
            let param_pos = analysis
                .parameters
                .c_parameters
                .iter()
                .enumerate()
                .filter_map(|(pos, orig_par)| {
                    if orig_par.name == mangled_par_name {
                        Some(pos)
                    } else {
                        None
                    }
                })
                .next()
                .unwrap();
            if array_lengths.contains(&(param_pos as u32)) {
                skip += 1;
                continue;
            }
        }

        if pos > skip {
            return_str.push_str(", ")
        }
        let s = out_parameter_as_return(par, env);
        return_str.push_str(&s);
    }
    return_str.push_str(&suffix);
    return_str
}

fn out_parameter_as_return(par: &library::Parameter, env: &Env) -> String {
    //TODO: upcasts?
    let rust_type = parameter_rust_type(
        env,
        par.typ,
        ParameterDirection::Return,
        par.nullable,
        RefMode::None,
        par.scope,
    );
    let name = rust_type.into_string();
    match ConversionType::of(env, par.typ) {
        ConversionType::Unknown => format!("/*Unknown conversion*/{}", name),
        _ => name,
    }
}
