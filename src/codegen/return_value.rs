use analysis;
use analysis::ref_mode::RefMode;
use env::Env;
use library::{self, ParameterDirection};
use analysis::conversion_type::ConversionType;
use analysis::rust_type::parameter_rust_type;
use traits::*;
use nameutil;

pub trait ToReturnValue {
    fn to_return_value(&self, env: &Env, is_trampoline: bool) -> String;
}

impl ToReturnValue for library::Parameter {
    fn to_return_value(&self, env: &Env, is_trampoline: bool) -> String {
        let rust_type =
            parameter_rust_type(env, self.typ, self.direction, self.nullable, RefMode::None);
        let mut name = rust_type.into_string();
        if is_trampoline && self.direction == library::ParameterDirection::Return && name == "GString" {
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
            Some(ref par) => par.to_return_value(env, is_trampoline),
            None => String::new(),
        }
    }
}

pub fn out_parameter_as_return_parts(
    analysis: &analysis::functions::Info,
) -> (&'static str, &'static str) {
    use analysis::out_parameters::Mode::*;
    let num_outs = analysis
        .outs
        .iter()
        .filter(|p| p.array_length.is_none())
        .count();
    match analysis.outs.mode {
        Normal | Combined => if num_outs > 1 {
            ("(", ")")
        } else {
            ("", "")
        },
        Optional => if num_outs > 1 {
            ("Option<(", ")>")
        } else {
            ("Option<", ">")
        },
        Throws(..) => {
            if num_outs == 1 + 1 {
                //if only one parameter except "glib::Error"
                ("Result<", ", Error>")
            } else {
                ("Result<(", "), Error>")
            }
        }
        None => unreachable!(),
    }
}

pub fn out_parameters_as_return(env: &Env, analysis: &analysis::functions::Info) -> String {
    let (prefix, suffix) = out_parameter_as_return_parts(analysis);
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
            let mangled_par_name = nameutil::mangle_keywords(par.name.as_str());
            let param_pos = analysis
                .parameters
                .c_parameters
                .iter()
                .enumerate()
                .filter_map(|(pos, orig_par)| if orig_par.name == mangled_par_name {
                    Some(pos)
                } else {
                    None
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
    return_str.push_str(suffix);
    return_str
}

pub fn out_parameter_as_return(par: &library::Parameter, env: &Env) -> String {
    //TODO: upcasts?
    let rust_type = parameter_rust_type(
        env,
        par.typ,
        ParameterDirection::Return,
        par.nullable,
        RefMode::None,
    );
    let name = rust_type.into_string();
    match ConversionType::of(env, par.typ) {
        ConversionType::Unknown => format!("/*Unknown conversion*/{}", name),
        _ => name,
    }
}
