use env::Env;
use library;

pub fn used_ffi_type(env: &Env, type_id: library::TypeId) -> Option<String> {
    use library::Type::*;
    use library::Fundamental::*;
    let type_ = env.library.type_(type_id);
    let ok = |s: &str| Some(format!("::{}", s));
    match  *type_ {
        Fundamental(fund) => {
            match fund {
                Boolean => ok("glib_ffi::gboolean"),
                Short => ok("libc::c_short"),
                UShort => ok("libc::c_ushort"),
                Int => ok("libc::c_int"),
                UInt => ok("libc::c_uint"),
                Long => ok("libc::c_long"),
                ULong => ok("libc::c_ulong"),
                Size => ok("libc::size_t"),
                SSize => ok("libc::ssize_t"),
                Float => ok("libc::c_float"),
                Double => ok("libc::c_double"),
                UChar => ok("libc::c_uchar"),
                Char |
                Utf8 |
                Filename => ok("libc::c_char"),
                Type => ok("glib_ffi::GType"),
                _ => Option::None,
            }
        }
        Alias(ref alias) => used_ffi_type(env, alias.typ)
            .and_then(|_| get_type_qualified_ffi_name(env, type_id, type_)),
        Bitfield(..) |
        Record(..) |
        Class(..) |
        Enumeration(..) |
        Interface(..) => get_type_qualified_ffi_name(env, type_id, type_),
        _ => Option::None,
    }
}

fn get_type_qualified_ffi_name(env: &Env, type_id: library::TypeId, type_: &library::Type) -> Option<String> {
    let name = if let Some(name) = type_.get_glib_name() {
        name
    } else {
        return None;
    };
    let ffi_crate_name = if type_id.ns_id == library::MAIN_NAMESPACE {
        "::ffi"
    } else {
        &env.namespaces[type_id.ns_id].ffi_crate_name[..]
    };
    Some(format!("{}::{}", ffi_crate_name, name))
}
