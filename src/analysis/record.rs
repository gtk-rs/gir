use std::ops::Deref;

use config::gobjects::GObject;
use env::Env;
use library;
use nameutil::*;
use super::*;
use super::imports::Imports;
use super::info_base::InfoBase;
use traits::*;

#[derive(Default)]
pub struct Info<'e> {
    pub base: InfoBase<'e>,
}

impl<'e> Deref for Info<'e> {
    type Target = InfoBase<'e>;

    fn deref(&self) -> &InfoBase<'e> {
        &self.base
    }
}

impl<'e> Info<'e> {
    //TODO: add test in tests/ for panic
    pub fn type_(&self, library: &'e library::Library) -> &'e library::Record {
        let type_ = library.type_(self.type_id).maybe_ref()
            .unwrap_or_else(|| panic!("{} is not a record.", self.full_name));
        type_
    }
}

pub fn new<'e>(env: &'e Env, obj: &GObject) -> Option<Info<'e>> {
    let full_name = obj.name.clone();

    let record_tid = match env.library.find_type(0, &full_name) {
        Some(tid) => tid,
        None => return None,
    };

    let type_ = env.type_(record_tid);

    let name: String = split_namespace_name(&full_name).1.into();

    let record: &library::Record = match type_.maybe_ref() {
        Some(record) => record,
        None => return None,
    };

    let mut imports = Imports::new();
    imports.add("glib::translate::*", None);
    imports.add("ffi", None);

    let mut functions = functions::analyze(env, &record.functions, record_tid, &obj, &mut imports);
    let specials = special_functions::extract(&mut functions);

    let (version, deprecated_version) = info_base::versions(env, &obj, &functions, record.version,
         record.deprecated_version);

    let is_shared = specials.get(&special_functions::Type::Ref).is_some() &&
        specials.get(&special_functions::Type::Unref).is_some();
    if is_shared {
        // `copy` will duplicate a struct while `clone` just adds a reference
        special_functions::unhide(&mut functions, &specials, special_functions::Type::Copy);
    };

    special_functions::analyze_imports(&specials, &mut imports);

    //don't `use` yourself
    imports.remove(&name);

    let base = InfoBase {
        full_name: full_name,
        type_id: record_tid,
        name: name,
        functions: functions,
        specials: specials,
        imports: imports,
        version: version,
        deprecated_version: deprecated_version,
        cfg_condition: obj.cfg_condition.clone(),
    };

    let info = Info {
        base: base,
    };

    Some(info)
}
