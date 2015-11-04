use env::Env;
use gobjects::GObject;
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

    let functions =
        functions::analyze(env, &record.functions, record_tid, &obj.non_nullable_overrides, &mut imports);

    let version = functions.iter().filter_map(|f| f.version).min();
    //TODO: remove copy, free, ref, unref (special functions)

    //don't `use` yourself
    imports.remove(&name);

    let info = Info {
        full_name: full_name,
        record_tid: record_tid,
        name: name,
        functions: functions,
        imports: imports,
        version: version,
    };

    Some(info)
}
