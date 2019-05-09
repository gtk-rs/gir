use crate::library::*;

pub trait FunctionsMutVisitor {
    //TODO: remove interrupt functionality if it is not used
    //visiting stops if returned false
    fn visit_function_mut(&mut self, func: &mut Function) -> bool;
}

impl Namespace {
    pub fn visit_functions_mut<V: FunctionsMutVisitor>(&mut self, visitor: &mut V) -> bool {
        for type_ in &mut self.types {
            if let Some(ref mut type_) = *type_ {
                if !type_.visit_functions_mut(visitor) {
                    return false;
                }
            }
        }
        true
    }
}

impl Type {
    pub fn visit_functions_mut<V: FunctionsMutVisitor>(&mut self, visitor: &mut V) -> bool {
        match *self {
            Type::Class(ref mut class) => for function in &mut class.functions {
                if !visitor.visit_function_mut(function) {
                    return false;
                }
            },
            Type::Interface(ref mut interface) => for function in &mut interface.functions {
                if !visitor.visit_function_mut(function) {
                    return false;
                }
            },
            Type::Record(ref mut record) => for function in &mut record.functions {
                if !visitor.visit_function_mut(function) {
                    return false;
                }
            },
            _ => (),
        }
        true
    }
}
