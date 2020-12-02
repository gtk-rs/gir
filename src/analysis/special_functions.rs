use crate::{
    analysis::{
        functions::{Info as FuncInfo, Visibility},
        imports::Imports,
    },
    library::{Type as LibType, TypeId},
    version::Version,
};
use std::{collections::BTreeMap, str::FromStr};

#[derive(Clone, Copy, Eq, Debug, Ord, PartialEq, PartialOrd)]
pub enum Type {
    Compare,
    Copy,
    Equal,
    Free,
    Ref,
    Display,
    Unref,
    Hash,
}

impl FromStr for Type {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use self::Type::*;
        match s {
            "compare" => Ok(Compare),
            "copy" => Ok(Copy),
            "equal" => Ok(Equal),
            "free" | "destroy" => Ok(Free),
            "is_equal" => Ok(Equal),
            "ref" | "ref_" => Ok(Ref),
            "to_string" => Ok(Display),
            "unref" => Ok(Unref),
            "hash" => Ok(Hash),
            _ => Err(format!("Unknown type '{}'", s)),
        }
    }
}

impl Type {
    fn extract(s: &str) -> Option<Self> {
        s.parse().ok().or_else(|| match s {
            "get_name" | "name" => Some(Self::Display),
            _ => None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Info {
    pub glib_name: String,
    pub returns_static_ref: bool,
    pub version: Option<Version>,
}

pub type Infos = BTreeMap<Type, Info>;

fn update_func(func: &mut FuncInfo, type_: Type, parent_type: &LibType) -> bool {
    if func.visibility != Visibility::Comment {
        func.visibility = visibility(type_);
    }

    if type_ == Type::Display {
        if func.parameters.c_parameters.len() != 1 {
            return false;
        }
        if !func.parameters.c_parameters[0].instance_parameter {
            return false;
        }
        if !func
            .ret
            .parameter
            .as_ref()
            .map_or(false, |p| p.typ == TypeId::tid_utf8())
        {
            return false;
        }

        if func.name == "to_string" {
            // Rename to to_str to make sure it doesn't clash with ToString::to_string
            func.name = "to_str".to_owned();

            // As to not change old code behaviour, assume non-nullability outside
            // enums and flags only. Function inside enums and flags have been
            // appropriately marked in Gir.
            if !matches!(parent_type, LibType::Enumeration(_) | LibType::Bitfield(_)) {
                if let Some(par) = func.ret.parameter.as_mut() {
                    *par.nullable = false;
                }
            }
        }

        // Cannot generate Display implementation for Option<>
        if func
            .ret
            .parameter
            .as_ref()
            .map_or(false, |ret| *ret.nullable)
        {
            return false;
        }
    }
    true
}

pub fn extract(functions: &mut Vec<FuncInfo>, parent_type: &LibType) -> Infos {
    let mut specials = Infos::new();
    let mut has_copy = false;
    let mut has_free = false;
    let mut destroy = None;

    for (pos, func) in functions.iter_mut().enumerate() {
        if let Some(type_) = Type::extract(&func.name) {
            if func.name == "destroy" {
                destroy = Some((func.glib_name.clone(), pos));
                continue;
            }
            if !update_func(func, type_, parent_type) {
                continue;
            }
            if func.name == "copy" {
                has_copy = true;
            } else if func.name == "free" {
                has_free = true;
            }

            let return_transfer_none = func.ret.parameter.as_ref().map_or(false, |ret| {
                ret.transfer == crate::library::Transfer::None
                // This is enforced already, otherwise no impl Display can be generated.
                && !*ret.nullable
            });

            // Assume only enumerations and bitfields can return static strings
            let returns_static_ref = type_ == Type::Display
                && return_transfer_none
                && matches!(parent_type, LibType::Enumeration(_) | LibType::Bitfield(_))
                // We cannot mandate returned lifetime if this is not generated.
                // (And this prevents an unused std::ffi::CStr from being emitted below)
                && func.status.need_generate();

            specials.insert(
                type_,
                Info {
                    glib_name: func.glib_name.clone(),
                    returns_static_ref,
                    version: func.version,
                },
            );
        }
    }

    if has_copy && !has_free {
        if let Some((glib_name, pos)) = destroy {
            let ty_ = Type::from_str("destroy").unwrap();
            let func = &mut functions[pos];
            update_func(func, ty_, parent_type);
            specials.insert(
                ty_,
                Info {
                    glib_name,
                    returns_static_ref: false,
                    version: func.version,
                },
            );
        }
    }

    specials
}

fn visibility(t: Type) -> Visibility {
    use self::Type::*;
    match t {
        Copy | Free | Ref | Unref => Visibility::Hidden,
        Hash | Compare | Equal => Visibility::Private,
        Display => Visibility::Public,
    }
}

// Some special functions (e.g. `copy` on refcounted types) should be exposed
pub fn unhide(functions: &mut Vec<FuncInfo>, specials: &Infos, type_: Type) {
    if let Some(func) = specials.get(&type_) {
        let func = functions
            .iter_mut()
            .find(|f| f.glib_name == func.glib_name && f.visibility != Visibility::Comment);
        if let Some(func) = func {
            func.visibility = Visibility::Public;
        }
    }
}

pub fn analyze_imports(specials: &Infos, imports: &mut Imports) {
    use self::Type::*;
    for (type_, info) in specials {
        match *type_ {
            Compare => imports.add_with_version("std::cmp", info.version),
            Display => {
                imports.add_with_version("std::fmt", info.version);
                if info.returns_static_ref {
                    imports.add_with_version("std::ffi::CStr", info.version);
                }
            }
            Hash => imports.add_with_version("std::hash", info.version),
            _ => {}
        }
    }
}
