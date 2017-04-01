use analysis::imports::Imports;
use analysis::ref_mode::RefMode;
use analysis::rust_type::*;
use analysis::signatures::{Signature, Signatures};
use config;
use config::gobjects::GObject;
use env::Env;
use library;
use nameutil;
use traits::*;
use version::Version;

#[derive(Debug)]
pub struct Property {
    pub name: String,
    pub var_name: String,
    pub typ: library::TypeId,
    pub is_get: bool,
    pub func_name: String,
    pub nullable: library::Nullable,
    pub conversion: PropertyConversion,
    pub default_value: Option<String>, //for getter
    pub get_out_ref_mode: RefMode,
    pub set_in_ref_mode: RefMode,
    pub version: Option<Version>,
    pub deprecated_version: Option<Version>,
}

pub fn analyze(env: &Env, props: &[library::Property], type_tid: library::TypeId,
               obj: &GObject, imports: &mut Imports, signatures: &Signatures,
               deps: &[library::TypeId]) -> Vec<Property> {
    let mut properties = Vec::new();

    for prop in props {
        let configured_properties = obj.properties.matched(&prop.name);
        if configured_properties.iter().any(|f| f.ignore) {
            continue;
        }

        if env.is_totally_deprecated(prop.deprecated_version) { continue; }

        let (getter, setter) = analyze_property(env, prop, type_tid, &configured_properties,
                                                signatures, deps);
        if getter.is_none() && setter.is_none() {
            continue;
        }

        let type_string = rust_type(env, prop.typ);
        let used_type_string = used_rust_type(env, prop.typ);
        if let Some(prop) = getter {
            if let Ok(ref s) = used_type_string {
                imports.add_used_type(s, prop.version);
            }
            if prop.conversion != PropertyConversion::Direct {
                imports.add("std::mem::transmute", prop.version);
            }
            if type_string.is_ok() && prop.default_value.is_some() {
                imports.add("glib::Value", prop.version);
                imports.add("gobject_ffi", prop.version);
            }

            properties.push(prop);
        }
        if let Some(prop) = setter {
            if let Ok(ref s) = used_type_string {
                imports.add_used_type(s, prop.version);
            }
            if type_string.is_ok() {
                imports.add("glib::Value", prop.version);
                imports.add("gobject_ffi", prop.version);
            }

            properties.push(prop);
        }
    }

    properties
}

fn analyze_property(env: &Env, prop: &library::Property, type_tid: library::TypeId,
                    configured_properties: &[&config::properties::Property],
                    signatures: &Signatures, deps: &[library::TypeId]) -> (Option<Property>, Option<Property>) {
    let name = prop.name.clone();
    let type_ = env.type_(prop.typ);

    let prop_version = configured_properties.iter().filter_map(|f| f.version).min()
        .or(prop.version);
    let name_for_func = nameutil::signal_to_snake(&name);
    let var_name = nameutil::mangle_keywords(&*name_for_func).into_owned();
    let get_func_name = format!("get_property_{}", name_for_func);
    let set_func_name = format!("set_property_{}", name_for_func);
    let check_get_func_name = format!("get_{}", name_for_func);
    let check_set_func_name = format!("set_{}", name_for_func);

    let mut readable = prop.readable;
    let mut writable = prop.writable;
    if prop.construct_only { writable = false; }

    if readable {
        let (has, version) = Signature::has_by_name_and_in_deps(env, &check_get_func_name,
                                                                signatures, deps);
        if has && (env.is_totally_deprecated(version) || version <= prop_version) {
            readable = false;
        }
    }
    if writable {
        let (has, version) = Signature::has_by_name_and_in_deps(env, &check_set_func_name,
                                                                signatures, deps);
        if has && (env.is_totally_deprecated(version) || version <= prop_version) {
            writable = false;
        }
    }

    let default_value = get_type_default_value(env, prop.typ, type_);
    if default_value.is_none() && readable {
        readable = false;
        let owner_name = rust_type(env, type_tid).into_string();
        error!("No default value for getter of property `{}` for `{}`", name, owner_name);
    }
    let conversion = PropertyConversion::of(type_);
    let get_out_ref_mode = RefMode::of(&env.library, prop.typ, library::ParameterDirection::Return);
    let mut set_in_ref_mode = RefMode::of(&env.library, prop.typ, library::ParameterDirection::In);
    if set_in_ref_mode == RefMode::ByRefMut {
        set_in_ref_mode = RefMode::ByRef;
    }
    let nullable = library::Nullable(set_in_ref_mode.is_ref());
    let getter = if readable {
        Some(Property{
            name: name.clone(),
            var_name: String::new(),
            typ: prop.typ,
            is_get: true,
            func_name: get_func_name,
            nullable: nullable,
            conversion: conversion,
            default_value: default_value,
            get_out_ref_mode: get_out_ref_mode,
            set_in_ref_mode: set_in_ref_mode,
            version: prop_version,
            deprecated_version: prop.deprecated_version,
        })
    } else { None };

    let setter = if writable {
        Some(Property{
            name: name.clone(),
            var_name: var_name,
            typ: prop.typ,
            is_get: false,
            func_name: set_func_name,
            nullable: nullable,
            conversion: conversion,
            default_value: None,
            get_out_ref_mode: get_out_ref_mode,
            set_in_ref_mode: set_in_ref_mode,
            version: prop_version,
            deprecated_version: prop.deprecated_version,
        })
    } else { None };

    (getter, setter)
}

pub fn get_type_default_value(env: &Env, type_tid: library::TypeId, type_: &library::Type)
                          -> Option<String> {
    use library::Type;
    use library::Fundamental;
    let some = |s: &str| Some(s.to_string());
    match *type_ {
        Type::Fundamental(fund) => {
            match fund {
                Fundamental::Boolean => some("&false"),
                Fundamental::Int => some("&0"),
                Fundamental::UInt => some("&0u32"),
                Fundamental::Utf8 => some("None::<&str>"),
                Fundamental::Float => some("&0f32"),
                Fundamental::Double => some("&0f64"),
                _ => None,
            }
        }
        Type::Bitfield(_) => some("&0u32"),
        Type::Enumeration(_) => some("&0"),
        Type::Class(..) |
        Type::Interface(..) => {
            let type_str = rust_type(env, type_tid).into_string();
            Some(format!("None::<&{}>", type_str))
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PropertyConversion {
    Direct,
    AsI32,
    Bitflag,
}

impl PropertyConversion {
    pub fn of(type_: &library::Type) -> PropertyConversion {
        use library::Type;
        use self::PropertyConversion::*;
        match *type_ {
            Type::Bitfield(_) => Bitflag,
            Type::Enumeration(_) => AsI32,
            _ => Direct,
        }
    }
}

impl Default for PropertyConversion {
    fn default() -> Self {
        PropertyConversion::Direct
    }
}
