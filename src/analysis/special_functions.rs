use std::collections::BTreeMap;
use std::str::FromStr;

use analysis::functions::Info as FuncInfo;

#[derive(Eq, Debug, Ord, PartialEq, PartialOrd)]
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

    functions.retain(|func| {
        match Type::from_str(&*func.name) {
            Ok(type_) => {
                specials.insert(type_, func.glib_name.clone());
                false
            }
            Err(_) => true,
        }
    });

    specials
}
