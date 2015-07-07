use analysis;
use env::Env;
use library;
use analysis::type_kind::TypeKind;
use analysis::rust_type::{AsStr, parameter_rust_type};

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
            match kind {
                TypeKind::Unknown => format_return(&format!("/*Unknown kind*/{}", name)),
                //TODO: records as in gtk_container_get_path_for_child
                TypeKind::Direct |
                    TypeKind::Converted |
                    TypeKind::Enumeration => format_return(&name),

                _ => format_return(&format!("Option<{}>", name)),
            }
        }
    }
}

impl ToReturnValue for Option<library::Parameter> {
    fn to_return_value(&self, env: &Env, func: &analysis::functions::Info) -> String {
        match self {
            &Some(ref par) => par.to_return_value(env, func),
            &None => String::new(),
        }
    }
}

fn format_return(type_str: &str) -> String {
    format!(" -> {}", type_str)
}
