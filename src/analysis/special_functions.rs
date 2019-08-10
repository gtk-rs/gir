use crate::analysis::{
    functions::{Info as FuncInfo, Visibility},
    imports::Imports,
};
use std::{collections::BTreeMap, str::FromStr};

#[derive(Clone, Copy, Eq, Debug, Ord, PartialEq, PartialOrd)]
pub enum Type {
    Compare,
    Copy,
    Equal,
    Free,
    Ref,
    ToString,
    Unref,
    Hash,
}

impl FromStr for Type {
    type Err = ();

    fn from_str(s: &str) -> Result<Type, ()> {
        use self::Type::*;
        match s {
            "compare" => Ok(Compare),
            "copy" => Ok(Copy),
            "equal" => Ok(Equal),
            "free" | "destroy" => Ok(Free),
            "is_equal" => Ok(Equal),
            "ref" | "ref_" => Ok(Ref),
            "to_string" => Ok(ToString),
            "unref" => Ok(Unref),
            "hash" => Ok(Hash),
            _ => Err(()),
        }
    }
}

pub type Infos = BTreeMap<Type, String>; // Type => glib_name

fn update_func(func: &mut FuncInfo, type_: Type) -> bool {
    if func.visibility != Visibility::Comment {
        func.visibility = visibility(type_, func.parameters.c_parameters.len());
    }
    // I assume `to_string` functions never return `NULL`
    if type_ == Type::ToString {
        if let Some(par) = func.ret.parameter.as_mut() {
            *par.nullable = false;
        }
        if func.visibility != Visibility::Private {
            return false;
        }
    }
    true
}

pub fn extract(functions: &mut Vec<FuncInfo>) -> Infos {
    let mut specials = BTreeMap::new();
    let mut has_copy = false;
    let mut has_free = false;
    let mut destroy = None;

    for (pos, func) in functions.iter_mut().enumerate() {
        if let Ok(type_) = Type::from_str(&func.name) {
            if func.name == "destroy" {
                destroy = Some((func.glib_name.clone(), pos));
                continue;
            }
            if !update_func(func, type_) {
                continue;
            }
            if func.name == "copy" {
                has_copy = true;
            } else if func.name == "free" {
                has_free = true;
            }
            specials.insert(type_, func.glib_name.clone());
        }
    }

    if has_copy && !has_free {
        if let Some((glib_name, pos)) = destroy {
            let ty_ = Type::from_str("destroy").unwrap();
            update_func(&mut functions[pos], ty_);
            specials.insert(ty_, glib_name);
        }
    }

    specials
}

fn visibility(t: Type, args_len: usize) -> Visibility {
    use self::Type::*;
    match t {
        Copy | Free | Ref | Unref => Visibility::Hidden,
        Hash | Compare | Equal => Visibility::Private,
        ToString if args_len == 1 => Visibility::Private,
        ToString => Visibility::Public,
    }
}

// Some special functions (e.g. `copy` on refcounted types) should be exposed
pub fn unhide(functions: &mut Vec<FuncInfo>, specials: &Infos, type_: Type) {
    if let Some(func) = specials.get(&type_) {
        let func = functions
            .iter_mut()
            .find(|f| f.glib_name == *func && f.visibility != Visibility::Comment);
        if let Some(func) = func {
            func.visibility = Visibility::Public;
        }
    }
}

pub fn analyze_imports(specials: &Infos, imports: &mut Imports) {
    use self::Type::*;
    for type_ in specials.keys() {
        match *type_ {
            Compare => imports.add("std::cmp"),
            ToString => imports.add("std::fmt"),
            Hash => imports.add("std::hash"),
            _ => {}
        }
    }
}
