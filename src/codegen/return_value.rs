use analysis;
use library;
use analysis::type_kind::{TypeKind, ToTypeKind};
use analysis::rust_type::ToRustType;

pub trait ToReturnValue {
    fn to_return_value(&self, library: &library::Library, func: &analysis::functions::Info) -> String;
}

impl ToReturnValue for library::Parameter {
    fn to_return_value(&self, library: &library::Library, func: &analysis::functions::Info) -> String {
        if func.kind == library::FunctionKind::Constructor {
            format_return(&func.class_name)
        } else {
            let type_ = library.type_(self.typ);
            let name = type_.to_rust_type();
            let kind = type_.to_type_kind(library);
            match kind {
                TypeKind::Unknown => format_return(&format!("/*Unknown kind*/{}", name)),
                TypeKind::None => String::new(),
                //TODO: records as in gtk_container_get_path_for_child
                TypeKind::Simple |
                    TypeKind::Enumeration => format_return(&name),

                _ => format_return(&format!("Option<{}>", name)),
            }
        }
    }
}

fn format_return(type_str: &str) -> String {
    format!(" -> {}", type_str)
}
