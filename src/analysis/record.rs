use std::ops::Deref;

use log::info;

use super::{imports::Imports, info_base::InfoBase, record_type::RecordType, *};
use crate::{
    config::{
        derives::{Derive, Derives},
        gobjects::GObject,
    },
    env::Env,
    library,
    nameutil::*,
    traits::*,
    version::Version,
};

#[derive(Debug, Default)]
pub struct Info {
    pub base: InfoBase,
    pub glib_get_type: Option<(String, Option<Version>)>,
    pub is_boxed: bool,
    pub derives: Derives,
    pub boxed_inline: bool,
    pub init_function_expression: Option<String>,
    pub copy_into_function_expression: Option<String>,
    pub clear_function_expression: Option<String>,
}

impl Deref for Info {
    type Target = InfoBase;

    fn deref(&self) -> &InfoBase {
        &self.base
    }
}

impl Info {
    // TODO: add test in tests/ for panic
    pub fn type_<'a>(&self, library: &'a library::Library) -> &'a library::Record {
        let type_ = library
            .type_(self.type_id)
            .maybe_ref()
            .unwrap_or_else(|| panic!("{} is not a record.", self.full_name));
        type_
    }
}

fn filter_derives(derives: &[Derive], names: &[&str]) -> Derives {
    derives
        .iter()
        .filter_map(|derive| {
            let new_names = derive
                .names
                .iter()
                .filter(|n| !names.contains(&n.as_str()))
                .map(Clone::clone)
                .collect::<Vec<_>>();

            if !new_names.is_empty() {
                Some(Derive {
                    names: new_names,
                    cfg_condition: derive.cfg_condition.clone(),
                })
            } else {
                None
            }
        })
        .collect()
}

pub fn new(env: &Env, obj: &GObject) -> Option<Info> {
    info!("Analyzing record {}", obj.name);
    let full_name = obj.name.clone();

    let record_tid = env.library.find_type(0, &full_name)?;

    let type_ = env.type_(record_tid);

    let name: String = split_namespace_name(&full_name).1.into();

    let record: &library::Record = type_.maybe_ref()?;

    let is_boxed = matches!(
        RecordType::of(record),
        RecordType::Boxed | RecordType::AutoBoxed
    );
    let boxed_inline = obj.boxed_inline;

    let mut imports = Imports::with_defined(&env.library, &name);

    let mut functions = functions::analyze(
        env,
        &record.functions,
        Some(record_tid),
        false,
        is_boxed,
        obj,
        &mut imports,
        None,
        None,
    );
    let specials = special_functions::extract(&mut functions, type_, obj);

    let version = obj.version.or(record.version);
    let deprecated_version = record.deprecated_version;

    let is_shared = specials.has_trait(special_functions::Type::Ref)
        && specials.has_trait(special_functions::Type::Unref);
    if is_shared {
        // `copy` will duplicate a struct while `clone` just adds a reference
        special_functions::unhide(&mut functions, &specials, special_functions::Type::Copy);
    };

    let mut derives = if let Some(ref derives) = obj.derives {
        if boxed_inline
            && !derives.is_empty()
            && !derives
                .iter()
                .all(|ds| ds.names.is_empty() || ds.names.iter().all(|n| n == "Debug"))
        {
            panic!("Can't automatically derive traits other than `Debug` for BoxedInline records");
        }
        derives.clone()
    } else if !boxed_inline {
        let derives = vec![Derive {
            names: vec![
                "Debug".into(),
                "PartialEq".into(),
                "Eq".into(),
                "PartialOrd".into(),
                "Ord".into(),
                "Hash".into(),
            ],
            cfg_condition: None,
        }];

        derives
    } else {
        // boxed_inline
        vec![]
    };

    for special in specials.traits().keys() {
        match special {
            special_functions::Type::Compare => {
                derives = filter_derives(&derives, &["PartialOrd", "Ord", "PartialEq", "Eq"]);
            }
            special_functions::Type::Equal => {
                derives = filter_derives(&derives, &["PartialEq", "Eq"]);
            }
            special_functions::Type::Hash => {
                derives = filter_derives(&derives, &["Hash"]);
            }
            _ => (),
        }
    }

    special_functions::analyze_imports(&specials, &mut imports);

    let glib_get_type = if let Some(ref glib_get_type) = record.glib_get_type {
        let configured_functions = obj.functions.matched("get_type");
        let get_type_version = configured_functions
            .iter()
            .map(|f| f.version)
            .max()
            .flatten();

        Some((glib_get_type.clone(), get_type_version))
    } else {
        None
    };

    // Check if we have to make use of the GType and the generic
    // boxed functions.
    if !is_shared
        && (!specials.has_trait(special_functions::Type::Copy)
            || !specials.has_trait(special_functions::Type::Free))
    {
        if let Some((_, get_type_version)) = glib_get_type {
            // FIXME: Ideally we would update it here but that requires fixing *all* the
            // versions of functions in this and other types that use this type somewhere in
            // the signature. Similar code exists for before the analysis already but that
            // doesn't apply directly here.
            //
            // As the get_type function only has a version if explicitly configured let's
            // just panic here. It's easy enough for the user to move the
            // version configuration from the function to the type.
            assert!(
                get_type_version <= version,
                "Have to use get_type function for {full_name} but version is higher than for the type ({get_type_version:?} > {version:?})"
            );
        } else {
            error!("Missing memory management functions for {}", full_name);
        }
    }

    let base = InfoBase {
        full_name,
        type_id: record_tid,
        name,
        functions,
        specials,
        imports,
        version,
        deprecated_version,
        cfg_condition: obj.cfg_condition.clone(),
        concurrency: obj.concurrency,
        visibility: obj.visibility,
    };

    let info = Info {
        base,
        glib_get_type,
        derives,
        is_boxed,
        boxed_inline,
        init_function_expression: obj.init_function_expression.clone(),
        copy_into_function_expression: obj.copy_into_function_expression.clone(),
        clear_function_expression: obj.clear_function_expression.clone(),
    };

    Some(info)
}
