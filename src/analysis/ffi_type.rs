use analysis::c_type::{implements_c_type, rustify_pointers};
use analysis::rust_type::{Result, TypeError};
use env::Env;
use library::*;
use traits::*;

//todo: use ffi_type
pub fn used_ffi_type(env: &Env, type_id: TypeId) -> Option<String> {
    use library::Type::*;
    use library::Fundamental::*;
    let type_ = env.library.type_(type_id);
    let ok = |s: &str| Some(format!("::{}", s));
    match  *type_ {
        Fundamental(fund) => {
            match fund {
                None => ok("c_void"),
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

fn get_type_qualified_ffi_name(env: &Env, type_id: TypeId, type_: &Type) -> Option<String> {
    let name = if let Some(name) = type_.get_glib_name() {
        name
    } else {
        return None;
    };
    Some(format!("{}::{}", &env.namespaces[type_id.ns_id].ffi_crate_name[..], name))
}

pub fn ffi_type(env: &Env, tid: TypeId, c_type: &str) -> Result {
    let (ptr, inner) = rustify_pointers(c_type);
    let res = if ptr.is_empty() {
        if let Some(c_tid) = env.library.find_type(0, c_type) {
            // Fast track plain fundamental types avoiding some checks
            if env.library.type_(c_tid).maybe_ref_as::<Fundamental>().is_some() {
                match *env.library.type_(tid) {
                    Type::FixedArray(_, size) => {
                        ffi_inner(env, c_tid, c_type.into())
                            .map_any(|s| format!("[{}; {}]", s, size))
                    }
                    Type::Class(Class { c_type: ref expected, .. }) |
                            Type::Interface(Interface { c_type: ref expected, .. })
                            if c_type == "gpointer" => {
                        info!("[c:type `gpointer` instead of `*mut {}`, fixing]", expected);
                        ffi_inner(env, tid, expected)
                            .map_any(|s| format!("*mut {}", s))
                    }
                    _ => {
                        ffi_inner(env, c_tid, c_type.into())
                    }
                }
            }
            else { // c_type isn't fundamental
                ffi_inner(env, tid, &inner)
            }
        }
        else { // c_type doesn't match any type in the library by name
            ffi_inner(env, tid, &inner)
        }
    }
    else { // ptr not empty
        ffi_inner(env, tid, &inner)
            .map_any(|s| format!("{} {}", ptr, s))
    };
    trace!("ffi_type({:?}, {}) -> {:?}", tid, c_type, res);
    res
}

fn ffi_inner(env: &Env, tid: TypeId, inner: &str) -> Result {
    let typ = env.library.type_(tid);
    match *typ {
        Type::Fundamental(fund) => {
            use library::Fundamental::*;
            let inner = match fund {
                None => "c_void",
                Boolean => "gboolean",
                Int8 => "i8",
                UInt8 => "u8",
                Int16 => "i16",
                UInt16 => "u16",
                Int32 => "i32",
                UInt32 => "u32",
                Int64 => "i64",
                UInt64 => "u64",
                Char => "c_char",
                UChar => "c_uchar",
                Short => "c_short",
                UShort => "c_ushort",
                Int => "c_int",
                UInt => "c_uint",
                Long => "c_long",
                ULong => "c_ulong",
                Size => "size_t",
                SSize => "ssize_t",
                Float => "c_float",
                Double => "c_double",
                UniChar => "u32",
                Utf8 => "c_char",
                Filename => "c_char",
                Type => "GType",
                _ => return Err(TypeError::Unimplemented(inner.into())),
            };
            Ok(inner.into())
        }
        Type::Record(..) | Type::Alias(..) | Type::Function(..) => {
            if let Some(declared_c_type) = typ.get_glib_name() {
                if declared_c_type != inner {
                    let msg = format!("[c:type mismatch `{}` != `{}` of `{}`]",
                                      inner, declared_c_type, typ.get_name());
                    warn!("{}", msg);
                    return Err(TypeError::Mismatch(msg));
                }
            }
            else {
                warn!("Type `{}` missing c_type", typ.get_name());
            }
            fix_name(env, tid, &inner)
        }
        Type::CArray(inner_tid) => ffi_inner(env, inner_tid, inner),
        Type::FixedArray(inner_tid, size) => {
            ffi_inner(env, inner_tid, inner)
                .map_any(|s| format!("[{}; {}]", s, size))
        }
        Type::Array(..) | Type::PtrArray(..)
                | Type::List(..) | Type::SList(..) | Type::HashTable(..) => {
            fix_name(env, tid, &inner)
        }
        _ => {
            if let Some(glib_name) = env.library.type_(tid).get_glib_name() {
                if inner != glib_name {
                    if implements_c_type(env, tid, &inner) {
                        info!("[c:type {} of {} <: {}, fixing]", glib_name,
                            env.library.type_(tid).get_name(), inner);
                        fix_name(env, tid, &glib_name)
                    }
                    else {
                        let msg = format!("[c:type mismatch {} != {} of {}]", inner, glib_name,
                            env.library.type_(tid).get_name());
                        warn!("{}", msg);
                        Err(TypeError::Mismatch(msg))
                    }
                }
                else {
                    fix_name(env, tid, &inner)
                }
            }
            else {
                let msg = format!("[Missing glib_name of {}, can't match != {}]",
                                  env.library.type_(tid).get_name(), inner);
                warn!("{}", msg);
                Err(TypeError::Mismatch(msg))
            }
        }
    }
}

fn fix_name(env: &Env, type_id: TypeId, name: &str) -> Result {
    if type_id.ns_id == INTERNAL_NAMESPACE {
        match *env.library.type_(type_id) {
            Type::Array(..) | Type::PtrArray(..) |
            Type::List(..) | Type::SList(..) | Type::HashTable(..) =>
                Ok(format!("{}::{}", &env.namespaces[env.namespaces.glib_ns_id].ffi_crate_name, name)),
            _ => Ok(name.into()),
        }
    } else {
        let name_with_prefix = format!("{}::{}", &env.namespaces[type_id.ns_id].ffi_crate_name, name);
        if env.type_status_sys(&type_id.full_name(&env.library)).ignored() {
            Err(TypeError::Ignored(name_with_prefix))
        } else {
            Ok(name_with_prefix)
        }
    }
}
