use analysis::imports::Imports;
use analysis::ref_mode::RefMode;
use analysis::rust_type::*;
use config;
use env::Env;
use library;
use traits::*;

#[derive(Clone, Debug)]
pub struct ChildProperty {
    pub name: String,
    pub typ: library::TypeId,
    pub child_name: String,
    pub child_type: Option<library::TypeId>,
    pub nullable: library::Nullable,
    pub is_like_i32: bool,
    pub default_value: Option<String>, //for getter
    pub get_out_ref_mode: RefMode,
    pub set_in_ref_mode: RefMode,
}

pub type ChildProperties = Vec<ChildProperty>;

pub fn analyze(env: &Env, config: Option<&config::ChildProperties>, type_tid: library::TypeId,
               imports: &mut Imports) -> ChildProperties {
    let mut properties = Vec::new();
    if config.is_none() {
        return properties;
    }
    let config = config.unwrap();
    let child_name = config.child_name.as_ref().map(|s| &s[..]).unwrap_or("child");
    let child_type = config.child_type.as_ref().and_then(|ref name| env.library.find_type(0, &name));
    if config.child_type.is_some() && child_type.is_none() {
        let owner_name = rust_type(env, type_tid).into_string();
        let child_type: &str = config.child_type.as_ref().unwrap();
        error!("Bad child type `{}` for `{}`", child_type, owner_name);
        return properties;
    }

    for prop in &config.properties {
        if let Some(prop) = analyze_property(env, prop, &child_name, child_type, type_tid, imports) {
            properties.push(prop);
        }
    }

    if !properties.is_empty() {
        imports.add("glib::object::IsA", None);
        if let Some(s) = child_type.and_then(|typ| used_rust_type(env, typ).ok()) {
            imports.add(&s, None);
        }
    }

    properties
}

fn analyze_property(env: &Env, prop: &config::ChildProperty, child_name: &str,
                    child_type: Option<library::TypeId>, type_tid: library::TypeId,
                    imports: &mut Imports) -> Option<ChildProperty> {
    let name = prop.name.clone();
    if let Some(typ) = env.library.find_type(0, &prop.type_name) {
        let type_ = env.type_(typ);

        imports.add("glib::Value", None);
        if let Ok(s) = used_rust_type(env, typ) {
            imports.add(&s, None);
        }

        let default_value = get_type_default_value(type_);
        if default_value.is_none() {
            let owner_name = rust_type(env, type_tid).into_string();
            error!("No default value for type `{}` of child property `{}` for `{}`", &prop.type_name, name, owner_name);
        }
        let is_like_i32 = get_is_like_i32(type_);
        if is_like_i32 {
            imports.add("std::mem::transmute", None);
        }
        let get_out_ref_mode = RefMode::of(&env.library, typ, library::ParameterDirection::Return);
        let set_in_ref_mode = RefMode::of(&env.library, typ, library::ParameterDirection::In);
        let nullable = library::Nullable(set_in_ref_mode.is_ref());
        Some(ChildProperty{
            name: name,
            typ: typ,
            child_name: child_name.to_owned(),
            child_type: child_type,
            nullable: nullable,
            is_like_i32: is_like_i32,
            default_value: default_value,
            get_out_ref_mode: get_out_ref_mode,
            set_in_ref_mode: set_in_ref_mode,
        })
    } else {
        let owner_name = rust_type(env, type_tid).into_string();
        error!("Bad type `{}` of child property `{}` for `{}`", &prop.type_name, name, owner_name);
        None
    }
}

fn get_type_default_value(type_: &library::Type) -> Option<String> {
    use library::Type;
    use library::Fundamental;
    let some = |s: &str| Some(s.to_string());
    match *type_ {
        Type::Fundamental(fund) => {
            match fund {
                Fundamental::Boolean => some("&false"),
                Fundamental::Int => some("&0"),
                Fundamental::Utf8 => some("None::<&str>"),
                _ => None,
            }
        }
        Type::Enumeration(_) => some("&0"),
        _ => None,
    }
}

fn get_is_like_i32(type_: &library::Type) -> bool {
    use library::Type;
    match *type_ {
        Type::Enumeration(_) => true,
        _ => false,
    }
}
