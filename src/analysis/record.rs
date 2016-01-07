use config::gobjects::GObject;
use env::Env;
use library;
use nameutil::*;
use super::*;
use super::imports::Imports;
use traits::*;
use version::Version;

#[derive(Default)]
pub struct Info {
    pub full_name: String,
    pub record_tid: library::TypeId,
    pub name: String,
    pub functions: Vec<functions::Info>,
    pub specials: special_functions::Infos,
    pub imports: Imports,
    pub version: Option<Version>,
}

impl Info {
    //TODO: add test in tests/ for panic
    pub fn type_<'a>(&self, library: &'a library::Library) -> &'a library::Record {
        let type_ = library.type_(self.record_tid).maybe_ref()
            .unwrap_or_else(|| panic!("{} is not a record.", self.full_name));
        type_
    }
}

pub fn new(env: &Env, obj: &GObject) -> Option<Info> {
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
    imports.add("glib::translate::*".into(), None);
    imports.add("ffi".into(), None);

    let mut functions =
        functions::analyze(env, &record.functions, record_tid, &obj, &mut imports);

    let version = functions.iter().filter_map(|f| f.version).min();

    let specials = special_functions::extract(&mut functions);

    let is_shared = specials.get(&special_functions::Type::Ref).is_some() &&
        specials.get(&special_functions::Type::Unref).is_some();
    if is_shared {
        // `copy` will duplicate a struct while `clone` just adds a reference
        special_functions::unhide(&mut functions, &specials, special_functions::Type::Copy);
        // accept only boxed records
        return None;
    };

    //don't `use` yourself
    imports.remove(&name);

    let info = Info {
        full_name: full_name,
        record_tid: record_tid,
        name: name,
        functions: functions,
        specials: specials,
        imports: imports,
        version: version,
    };

    Some(info)
}
