use super::{imports::Imports, info_base::InfoBase, *};
use crate::{config::gobjects::GObject, env::Env, library, nameutil::*, traits::*};
use log::info;
use std::ops::Deref;

#[derive(Debug, Default)]
pub struct Info {
    pub base: InfoBase,
}

impl Deref for Info {
    type Target = InfoBase;

    fn deref(&self) -> &InfoBase {
        &self.base
    }
}

impl Info {
    pub fn type_<'a>(&self, library: &'a library::Library) -> &'a library::Enumeration {
        let type_ = library
            .type_(self.type_id)
            .maybe_ref()
            .unwrap_or_else(|| panic!("{} is not an enumeration.", self.full_name));
        type_
    }
}

pub fn new(env: &Env, obj: &GObject, imports: &mut Imports) -> Option<Info> {
    info!("Analyzing enumeration {}", obj.name);

    let enumeration_tid = env.library.find_type(0, &obj.name)?;
    let type_ = env.type_(enumeration_tid);
    let enumeration: &library::Enumeration = type_.maybe_ref()?;

    let name = split_namespace_name(&obj.name).1;

    let mut functions = functions::analyze(
        env,
        &enumeration.functions,
        enumeration_tid,
        false,
        false,
        obj,
        imports,
        None,
        None,
    );
    let specials = special_functions::extract(&mut functions);

    special_functions::analyze_imports(&specials, imports);

    let (version, deprecated_version) = info_base::versions(
        env,
        obj,
        &functions,
        enumeration.version,
        enumeration.deprecated_version,
    );

    let base = InfoBase {
        full_name: obj.name.clone(),
        type_id: enumeration_tid,
        name: name.to_owned(),
        functions,
        specials,
        // TODO: Don't use!
        imports: Imports::new(&env.library),
        version,
        deprecated_version,
        cfg_condition: obj.cfg_condition.clone(),
        concurrency: obj.concurrency,
    };

    let info = Info { base };

    Some(info)
}
