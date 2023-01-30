use std::{collections::BTreeMap, str::FromStr};

use crate::{
    analysis::{functions::Info as FuncInfo, imports::Imports},
    codegen::Visibility,
    config::GObject,
    library::{Type as LibType, TypeId},
    version::Version,
};

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
        match s {
            "compare" => Ok(Self::Compare),
            "copy" => Ok(Self::Copy),
            "equal" => Ok(Self::Equal),
            "free" | "destroy" => Ok(Self::Free),
            "is_equal" => Ok(Self::Equal),
            "ref" | "ref_" => Ok(Self::Ref),
            "unref" => Ok(Self::Unref),
            "hash" => Ok(Self::Hash),
            _ => Err(format!("Unknown type '{s}'")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraitInfo {
    pub glib_name: String,
    pub version: Option<Version>,
    pub first_parameter_mut: bool,
}

type TraitInfos = BTreeMap<Type, TraitInfo>;

#[derive(Clone, Copy, Eq, Debug, Ord, PartialEq, PartialOrd)]
pub enum FunctionType {
    StaticStringify,
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub type_: FunctionType,
    pub version: Option<Version>,
}

type FunctionInfos = BTreeMap<String, FunctionInfo>;

#[derive(Debug, Default)]
pub struct Infos {
    traits: TraitInfos,
    functions: FunctionInfos,
}

impl Infos {
    pub fn traits(&self) -> &TraitInfos {
        &self.traits
    }

    pub fn traits_mut(&mut self) -> &mut TraitInfos {
        &mut self.traits
    }

    pub fn has_trait(&self, type_: Type) -> bool {
        self.traits.contains_key(&type_)
    }

    pub fn functions(&self) -> &FunctionInfos {
        &self.functions
    }
}

/// Returns true on functions that take an instance as single argument and
/// return a string as result.
fn is_stringify(func: &mut FuncInfo, parent_type: &LibType, obj: &GObject) -> bool {
    if func.parameters.c_parameters.len() != 1 {
        return false;
    }
    if !func.parameters.c_parameters[0].instance_parameter {
        return false;
    }

    if let Some(ret) = func.ret.parameter.as_mut() {
        if ret.lib_par.typ != TypeId::tid_utf8() {
            return false;
        }

        if func.name == "to_string" {
            // Rename to to_str to make sure it doesn't clash with ToString::to_string
            assert!(func.new_name.is_none(), "A `to_string` function can't be renamed manually. It's automatically renamed to `to_str`");
            func.new_name = Some("to_str".to_owned());

            // As to not change old code behaviour, assume non-nullability outside
            // enums and flags only, and exclusively for to_string. Function inside
            // enums and flags have been appropriately marked in Gir.
            if !obj.trust_return_value_nullability
                && !matches!(parent_type, LibType::Enumeration(_) | LibType::Bitfield(_))
            {
                *ret.lib_par.nullable = false;
            }
        }

        // Cannot generate Display implementation for Option<>
        !*ret.lib_par.nullable
    } else {
        false
    }
}

fn update_func(func: &mut FuncInfo, type_: Type) -> bool {
    if !func.commented {
        use self::Type::*;
        match type_ {
            Copy | Free | Ref | Unref => func.hidden = true,
            Hash | Compare | Equal => func.visibility = Visibility::Private,
            Display => func.visibility = Visibility::Public,
        };
    }
    true
}

pub fn extract(functions: &mut [FuncInfo], parent_type: &LibType, obj: &GObject) -> Infos {
    let mut specials = Infos::default();
    let mut has_copy = false;
    let mut has_free = false;
    let mut destroy = None;

    for (pos, func) in functions.iter_mut().enumerate() {
        if is_stringify(func, parent_type, obj) {
            let return_transfer_none = func.ret.parameter.as_ref().map_or(false, |ret| {
                ret.lib_par.transfer == crate::library::Transfer::None
            });

            // Assume only enumerations and bitfields can return static strings
            let returns_static_ref = return_transfer_none
                && matches!(parent_type, LibType::Enumeration(_) | LibType::Bitfield(_))
                // We cannot mandate returned lifetime if this is not generated.
                // (And this prevents an unused glib::GStr from being emitted below)
                && func.status.need_generate();

            if returns_static_ref {
                // Override the function with a &'static (non allocating) -returning string
                // if the transfer type is none and it matches the above heuristics.
                specials.functions.insert(
                    func.glib_name.clone(),
                    FunctionInfo {
                        type_: FunctionType::StaticStringify,
                        version: func.version,
                    },
                );
            }

            // Some stringifying functions can serve as Display implementation
            if matches!(
                func.name.as_str(),
                "to_string" | "to_str" | "name" | "get_name"
            ) {
                // FUTURE: Decide which function gets precedence if multiple Display prospects
                // exist.
                specials.traits.insert(
                    Type::Display,
                    TraitInfo {
                        glib_name: func.glib_name.clone(),
                        version: func.version,
                        first_parameter_mut: false,
                    },
                );
            }
        } else if let Ok(type_) = func.name.parse() {
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

            let first_parameter_mut = func
                .parameters
                .c_parameters
                .first()
                .map_or(false, |p| p.ref_mode == super::ref_mode::RefMode::ByRefMut);

            specials.traits.insert(
                type_,
                TraitInfo {
                    glib_name: func.glib_name.clone(),
                    version: func.version,
                    first_parameter_mut,
                },
            );
        }
    }

    if has_copy && !has_free {
        if let Some((glib_name, pos)) = destroy {
            let ty_ = Type::from_str("destroy").unwrap();
            let func = &mut functions[pos];
            update_func(func, ty_);
            specials.traits.insert(
                ty_,
                TraitInfo {
                    glib_name,
                    version: func.version,
                    first_parameter_mut: true,
                },
            );
        }
    }

    specials
}

// Some special functions (e.g. `copy` on refcounted types) should be exposed
pub fn unhide(functions: &mut [FuncInfo], specials: &Infos, type_: Type) {
    if let Some(func) = specials.traits().get(&type_) {
        let func = functions
            .iter_mut()
            .find(|f| f.glib_name == func.glib_name && !f.commented);
        if let Some(func) = func {
            func.visibility = Visibility::Public;
            func.hidden = false;
        }
    }
}

pub fn analyze_imports(specials: &Infos, imports: &mut Imports) {
    for (type_, info) in specials.traits() {
        use self::Type::*;
        match type_ {
            Copy if info.first_parameter_mut => {
                imports.add_with_version("glib::translate::*", info.version);
            }
            Compare => {
                imports.add_with_version("std::cmp", info.version);
                imports.add_with_version("glib::translate::*", info.version);
            }
            Display => imports.add_with_version("std::fmt", info.version),
            Hash => imports.add_with_version("std::hash", info.version),
            Equal => imports.add_with_version("glib::translate::*", info.version),
            _ => {}
        }
    }
    for info in specials.functions().values() {
        match info.type_ {
            FunctionType::StaticStringify => {
                imports.add_with_version("glib::GStr", info.version);
            }
        }
    }
}
