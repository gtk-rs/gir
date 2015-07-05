use analysis;
use library;
use analysis::type_kind::{TypeKind, ToTypeKind};
use analysis::rust_type::{AsStr, parameter_rust_type};

pub trait ToReturnValue {
    fn to_return_value(&self, library: &library::Library, func: &analysis::functions::Info) -> String;
}

impl ToReturnValue for library::Parameter {
    fn to_return_value(&self, library: &library::Library, func: &analysis::functions::Info) -> String {
        if func.kind == library::FunctionKind::Constructor {
            format_return(&func.class_name.as_str())
        } else {
            let type_ = library.type_(self.typ);
            let rust_type = parameter_rust_type(library, self.typ, self.direction);
            let name = rust_type.as_str();
            let kind = type_.to_type_kind(library);
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
    fn to_return_value(&self, library: &library::Library, func: &analysis::functions::Info) -> String {
        match self {
            &Some(ref par) => par.to_return_value(library, func),
            &None => String::new(),
        }
    }
}

fn format_return(type_str: &str) -> String {
    format!(" -> {}", type_str)
}
