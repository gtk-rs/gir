use config::Config;
use library::{self, Function, Parameter, Type, MAIN_NAMESPACE};
use Library;
use version::Version;

pub fn apply_config(library: &mut Library, cfg: &Config) {
    fix_versions_by_config(library, cfg);
}

pub fn check_function_real_version(library: &mut Library) {
    // In order to avoid the borrow checker to annoy us...
    let library2 = library as *const Library;
    for typ in &mut library.namespace_mut(MAIN_NAMESPACE).types {
        match *typ {
            Some(Type::Class(ref mut c)) => update_function_version(&mut c.functions, library2),
            Some(Type::Interface(ref mut i)) => update_function_version(&mut i.functions, library2),
            Some(Type::Union(ref mut u)) => update_function_version(&mut u.functions, library2),
            Some(Type::Record(ref mut r)) => update_function_version(&mut r.functions, library2),
            Some(Type::Bitfield(ref mut b)) => update_function_version(&mut b.functions, library2),
            Some(Type::Enumeration(ref mut e)) => {
                update_function_version(&mut e.functions, library2)
            }
            _ => {}
        }
    }
    update_function_version(
        &mut library.namespace_mut(MAIN_NAMESPACE).functions,
        library2,
    );
}

fn check_versions(param: &Parameter, current_version: &mut Option<Version>, lib: *const Library) {
    if param.typ.ns_id != MAIN_NAMESPACE {
        return;
    }
    let ty_version = match *unsafe { (*lib).type_(param.typ) } {
        library::Type::Class(ref c) => c.version,
        library::Type::Enumeration(ref c) => c.version,
        library::Type::Bitfield(ref c) => c.version,
        library::Type::Record(ref c) => c.version,
        library::Type::Interface(ref c) => c.version,
        _ => None,
    };
    let new_version = match (*current_version, ty_version) {
        (Some(current_version), Some(ty_version)) => if current_version < ty_version {
            Some(ty_version)
        } else {
            None
        },
        (None, Some(ty_version)) => Some(ty_version),
        _ => None,
    };
    if let Some(new_version) = new_version {
        *current_version = Some(new_version);
    }
}

fn update_function_version(functions: &mut Vec<Function>, lib: *const Library) {
    for function in functions {
        let mut current_version = None;
        for parameter in &function.parameters {
            check_versions(parameter, &mut current_version, lib);
        }
        check_versions(&function.ret, &mut current_version, lib);
        if match (current_version, function.version) {
            (Some(cur_ver), Some(lib_ver)) => cur_ver > lib_ver,
            (Some(_), None) => true,
            _ => false,
        } {
            function.version = current_version;
        }
    }
}

fn fix_versions_by_config(library: &mut Library, cfg: &Config) {
    use library::Type::*;
    for obj in cfg.objects.values() {
        if obj.status.ignored() {
            continue;
        }
        if obj.version.is_none() {
            continue;
        }
        let version = obj.version;

        let tid = match library.find_type(0, &obj.name) {
            Some(x) => x,
            None => continue,
        };
        match *library.type_mut(tid) {
            Class(ref mut class) => class.version = version,
            Interface(ref mut interface) => interface.version = version,
            Record(ref mut record) => record.version = version,
            Bitfield(ref mut flags) => flags.version = version,
            Enumeration(ref mut enum_) => enum_.version = version,
            _ => (),
        }
    }
}
