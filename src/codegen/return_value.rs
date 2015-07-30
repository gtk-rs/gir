use analysis;
use env::Env;
use library;
use analysis::type_kind::TypeKind;
use analysis::rust_type::parameter_rust_type;
use traits::*;

pub trait ToReturnValue {
    fn to_return_value(&self, env: &Env, func: &analysis::functions::Info) -> String;
}

impl ToReturnValue for library::Parameter {
    fn to_return_value(&self, env: &Env, func: &analysis::functions::Info) -> String {
        if func.kind == library::FunctionKind::Constructor {
            format_return(&func.class_name.as_str())
        } else {
            let rust_type = parameter_rust_type(env, self.typ, self.direction);
            let name = rust_type.as_str();
            let kind = TypeKind::of(&env.library, self.typ);
            let type_str = match kind {
                TypeKind::Unknown => format!("/*Unknown kind*/{}", name),
                //TODO: records as in gtk_container_get_path_for_child
                TypeKind::Direct |
                    TypeKind::Enumeration => name.into(),

                _ => maybe_optional(name, self)
            };
            format_return(&type_str)
        }
    }
}

impl ToReturnValue for analysis::return_value::Info {
    fn to_return_value(&self, env: &Env, func: &analysis::functions::Info) -> String {
        match self.parameter {
            Some(ref par) => par.to_return_value(env, func),
            None => String::new(),
        }
    }
}

fn format_return(type_str: &str) -> String {
    format!(" -> {}", type_str)
}

fn maybe_optional(name: &str, par: &library::Parameter) -> String {
    if par.nullable {
        format!("Option<{}>", name)
    } else {
        name.into()
    }
}

pub fn out_parameters_as_return(env: &Env, analysis: &analysis::functions::Info) -> String {
    let (prefix, suffix) = if analysis.outs.len() > 1 { ("(", ")") } else { ("", "") };
    let mut return_str = String::with_capacity(100);
    return_str.push_str(" -> ");
    return_str.push_str(prefix);
    for (pos, par) in analysis.outs.iter().enumerate() {
        if pos > 0 { return_str.push_str(", ") }
        let s = out_parameter_as_return(par, env);
        return_str.push_str(&s);
    }
    return_str.push_str(suffix);
    return_str
}

fn out_parameter_as_return(par: &library::Parameter, env: &Env) -> String {
    //TODO: upcasts?
    let rust_type = parameter_rust_type(env, par.typ, library::ParameterDirection::Return);
    let name = rust_type.as_str();
    let kind = TypeKind::of(&env.library, par.typ);
    match kind {
        TypeKind::Unknown => format!("/*Unknown kind*/{}", name),

        TypeKind::Direct |
            TypeKind::Converted |
            TypeKind::Enumeration => name.into(),

        _ => format!("Option<{}>", name),
    }
}
