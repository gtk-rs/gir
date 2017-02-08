use std::collections::BTreeMap;
use std::str::FromStr;

use analysis::functions::Info as FuncInfo;
use analysis::functions::Visibility;
use analysis::imports::Imports;

#[derive(Clone, Copy, Eq, Debug, Ord, PartialEq, PartialOrd)]
pub enum Type {
    Compare,
    Copy,
    Equal,
    Free,
    Ref,
    ToString,
    Unref,
}

impl FromStr for Type {
    type Err = ();

    fn from_str(s: &str) -> Result<Type, ()> {
        use self::Type::*;
        match s {
            "compare" => Ok(Compare),
            "copy" => Ok(Copy),
            "equal" => Ok(Equal),
            "free" => Ok(Free),
            "is_equal" => Ok(Equal),
            "ref" | "ref_" => Ok(Ref),
            "to_string" => Ok(ToString),
            "unref" => Ok(Unref),
            _ => Err(()),
        }
    }
}

pub type Infos = BTreeMap<Type, String>; //Type => glib_name

pub fn extract(functions: &mut Vec<FuncInfo>) -> Infos {
    let mut specials = BTreeMap::new();

    for func in functions.iter_mut() {
        if let Ok(type_) = Type::from_str(&func.name) {
            if func.visibility != Visibility::Comment {
                func.visibility = visibility(type_);
            }
            // I assume `to_string` functions never return `NULL`
            if type_ == Type::ToString {
                if let Some(par) = func.ret.parameter.as_mut() {
                    *par.nullable = false;
                }
                if func.parameters.len() != 1 {
                    continue;
                }
            }
            specials.insert(type_, func.glib_name.clone());
        }
    }

    specials
}

fn visibility(t: Type) -> Visibility {
    use self::Type::*;
    match t {
        Copy |
            Free |
            Ref |
            Unref => Visibility::Hidden,
        Compare |
            Equal |
            ToString => Visibility::Private,
    }
}

// Some special functions (e.g. `copy` on refcounted types) should be exposed
pub fn unhide(functions: &mut Vec<FuncInfo>, specials: &Infos, type_: Type) {
    if let Some(func) = specials.get(&type_) {
        let func = functions.iter_mut()
            .filter(|f| f.glib_name == *func && f.visibility != Visibility::Comment)
            .next();
        if let Some(func) = func {
            func.visibility = Visibility::Public;
        }
    }
}

pub fn analyze_imports(specials: &Infos, imports: &mut Imports) {
    use self::Type::*;
    for type_ in specials.keys() {
        match *type_ {
            Compare => imports.add("std::cmp", None),
            ToString => imports.add("std::fmt", None),
            _ => {}
        }
    }
}
