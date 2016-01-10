use analysis;
use analysis::ref_mode::RefMode;
use env::Env;
use library::{self, ParameterDirection};
use analysis::conversion_type::ConversionType;
use analysis::rust_type::parameter_rust_type;
use traits::*;

pub trait ToReturnValue {
    fn to_return_value(&self, env: &Env) -> String;
}

impl ToReturnValue for library::Parameter {
    fn to_return_value(&self, env: &Env) -> String {
        let rust_type = parameter_rust_type(env, self.typ, self.direction,
                                            self.nullable, RefMode::None);
        let name = rust_type.as_str();
        let type_str = match ConversionType::of(&env.library, self.typ) {
            ConversionType::Unknown => format!("/*Unknown conversion*/{}", name),
            //TODO: records as in gtk_container_get_path_for_child
            _ => name.into(),
        };
        format!(" -> {}", type_str)
    }
}

impl ToReturnValue for analysis::return_value::Info {
    fn to_return_value(&self, env: &Env) -> String {
        match self.parameter {
            Some(ref par) => par.to_return_value(env),
            None => String::new(),
        }
    }
}

pub fn out_parameter_as_return_parts(analysis: &analysis::functions::Info)
                                     -> (&'static str, &'static str) {
    use analysis::out_parameters::Mode::*;
    let is_tuple = analysis.outs.len() > 1;
    match analysis.outs.mode {
        Normal |
            Combined => if is_tuple { ("(", ")") } else { ("", "") },
        Optional => if is_tuple { ("Option<(", ")>") } else { ("Option<", ">") },
        Throws(..) => if analysis.outs.len() == 1 + 1 {
            //if only one parameter except "glib::Error"
            ("Result<", ", glib::Error>")
        } else {
            ("Result<(", "), glib::Error>")
        },
        None => unreachable!(),
    }
}

pub fn out_parameters_as_return(env: &Env, analysis: &analysis::functions::Info) -> String {
    let (prefix, suffix) = out_parameter_as_return_parts(analysis);
    let mut return_str = String::with_capacity(100);
    return_str.push_str(" -> ");
    return_str.push_str(prefix);
    for (pos, par) in analysis.outs.iter().filter(|par| !par.is_error).enumerate() {
        if pos > 0 { return_str.push_str(", ") }
        let s = out_parameter_as_return(par, env);
        return_str.push_str(&s);
    }
    return_str.push_str(suffix);
    return_str
}

fn out_parameter_as_return(par: &library::Parameter, env: &Env) -> String {
    //TODO: upcasts?
    let rust_type = parameter_rust_type(env, par.typ, ParameterDirection::Return,
                                        par.nullable, RefMode::None);
    let name = rust_type.as_str();
    match ConversionType::of(&env.library, par.typ) {
        ConversionType::Unknown => format!("/*Unknown conversion*/{}", name),
        _ => name.into(),
    }
}
