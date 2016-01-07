use std::collections::BTreeMap;
use std::str::FromStr;

use analysis::functions::Info as FuncInfo;
use analysis::functions::Visibility;

#[derive(Clone, Copy, Eq, Debug, Ord, PartialEq, PartialOrd)]
pub enum Type {
    Copy,
    Free,
    Ref,
    Unref,
}

impl FromStr for Type {
    type Err = ();

    fn from_str(s: &str) -> Result<Type, ()> {
        use self::Type::*;
        match s {
            "copy" => Ok(Copy),
            "free" => Ok(Free),
            "ref" => Ok(Ref),
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
