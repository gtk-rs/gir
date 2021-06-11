use crate::{
    analysis::{bounds::Bounds, imports::Imports, ref_mode::RefMode, rust_type::RustType},
    codegen::function,
    config,
    env::Env,
    library::{self, ParameterDirection},
    nameutil,
    traits::*,
};
use log::error;

#[derive(Clone, Debug)]
pub struct ChildProperty {
    pub name: String,
    pub prop_name: String,
    pub getter_name: String,
    pub typ: library::TypeId,
    pub child_name: String,
    pub child_type: Option<library::TypeId>,
    pub nullable: library::Nullable,
    pub get_out_ref_mode: RefMode,
    pub set_in_ref_mode: RefMode,
    pub doc_hidden: bool,
    pub set_params: String,
    pub bounds: String,
    pub to_glib_extra: String,
}

pub type ChildProperties = Vec<ChildProperty>;

pub fn analyze(
    env: &Env,
    config: Option<&config::ChildProperties>,
    type_tid: library::TypeId,
    imports: &mut Imports,
) -> ChildProperties {
    let mut properties = Vec::new();
    if config.is_none() {
        return properties;
    }
    let config = config.unwrap();
    let child_name = config
        .child_name
        .as_ref()
        .map(|s| &s[..])
        .unwrap_or("child");
    let child_type = config
        .child_type
        .as_ref()
        .and_then(|name| env.library.find_type(0, name));
    if config.child_type.is_some() && child_type.is_none() {
        let owner_name = RustType::try_new(env, type_tid).into_string();
        let child_type: &str = config.child_type.as_ref().unwrap();
        error!("Bad child type `{}` for `{}`", child_type, owner_name);
        return properties;
    }

    for prop in &config.properties {
        if let Some(prop) =
            analyze_property(env, prop, child_name, child_type, type_tid, config, imports)
        {
            properties.push(prop);
        }
    }

    if !properties.is_empty() {
        imports.add("glib::object::IsA");
        if let Some(rust_type) = child_type.and_then(|typ| RustType::try_new(env, typ).ok()) {
            imports.add_used_types(rust_type.used_types());
        }
    }

    properties
}

fn analyze_property(
    env: &Env,
    prop: &config::ChildProperty,
    child_name: &str,
    child_type: Option<library::TypeId>,
    type_tid: library::TypeId,
    config: &config::ChildProperties,
    imports: &mut Imports,
) -> Option<ChildProperty> {
    let name = prop.name.clone();
    let prop_name = nameutil::signal_to_snake(&*prop.name);
    let getter_rename = config
        .properties
        .iter()
        .find(|cp| cp.name == name)
        .and_then(|cp| cp.rename_getter.clone());
    let is_getter_renamed = getter_rename.is_some();
    let mut getter_name = getter_rename.unwrap_or_else(|| prop_name.clone());

    if let Some(typ) = env.library.find_type(0, &prop.type_name) {
        let doc_hidden = prop.doc_hidden;

        imports.add("glib::StaticType");
        if let Ok(rust_type) = RustType::try_new(env, typ) {
            imports.add_used_types(rust_type.used_types());
        }

        let get_out_ref_mode = RefMode::of(env, typ, library::ParameterDirection::Return);
        if !is_getter_renamed {
            if let Ok(new_name) = getter_rules::try_rename_getter_suffix(
                &getter_name,
                typ == library::TypeId::tid_bool(),
            ) {
                getter_name = new_name.unwrap();
            }
        }

        let mut set_in_ref_mode = RefMode::of(env, typ, library::ParameterDirection::In);
        if set_in_ref_mode == RefMode::ByRefMut {
            set_in_ref_mode = RefMode::ByRef;
        }
        let nullable = library::Nullable(set_in_ref_mode.is_ref());

        let mut bounds_str = String::new();
        let dir = ParameterDirection::In;

        let set_params = if let Some(bound) = Bounds::type_for(env, typ, nullable) {
            let r_type = RustType::builder(env, typ)
                .ref_mode(RefMode::ByRefFake)
                .try_build()
                .into_string();
            let mut bounds = Bounds::default();
            bounds.add_parameter("P", &r_type, bound, false);
            let (s_bounds, _) = function::bounds(&bounds, &[], false, false);
            // Because the bounds won't necessarily be added into the final function, we
            // only keep the "inner" part to make the string computation easier. So
            // `<T: X>` becomes `T: X`.
            bounds_str.push_str(&s_bounds[1..s_bounds.len() - 1]);
            format!("{}: {}", prop_name, bounds.iter().last().unwrap().alias)
        } else {
            format!(
                "{}: {}",
                prop_name,
                RustType::builder(env, typ)
                    .direction(dir)
                    .nullable(nullable)
                    .ref_mode(set_in_ref_mode)
                    .try_build_param()
                    .into_string()
            )
        };

        Some(ChildProperty {
            name,
            prop_name,
            getter_name,
            typ,
            child_name: child_name.to_owned(),
            child_type,
            nullable,
            get_out_ref_mode,
            set_in_ref_mode,
            doc_hidden,
            set_params,
            bounds: bounds_str,
            to_glib_extra: String::new(),
        })
    } else {
        let owner_name = RustType::try_new(env, type_tid).into_string();
        error!(
            "Bad type `{}` of child property `{}` for `{}`",
            &prop.type_name, name, owner_name
        );
        None
    }
}
