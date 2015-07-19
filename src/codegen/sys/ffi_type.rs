use analysis::rust_type::Result;
use env::Env;
use library;
use library::*;
use nameutil::crate_name;
use traits::*;

// FIXME: This module needs redundant allocations audit
// TODO: ffi_type computations should be cached

pub fn ffi_type(env: &Env, tid: library::TypeId, c_type: &str) -> Result {
    let (ptr, inner) = rustify_pointers(c_type);
    let res = if ptr.is_empty() {
        if let Some(c_tid) = env.library.find_type(0, c_type) {
            // Fast track plain fundamental types avoiding some checks
            if env.library.type_(c_tid).maybe_ref_as::<Fundamental>().is_some() {
                if let &library::Type::FixedArray(_, size) = env.library.type_(tid) {
                    ffi_inner(env, c_tid, c_type.into())
                        .map_any(|s| format!("[{}; {}]", s, size))
                }
                else {
                    ffi_inner(env, c_tid, c_type.into())
                }
            }
            else { // c_type isn't fundamental
                ffi_inner(env, tid, inner)
            }
        }
        else { // c_type doesn't match any type in the library by name
            ffi_inner(env, tid, inner)
        }
    }
    else { // ptr not empty
        ffi_inner(env, tid, inner)
            .map_any(|s| format!("{} {}", ptr, s))
    };
    trace!("ffi_type({:?}, {}) -> {:?}", tid, c_type, res);
    res
}

fn ffi_inner(env: &Env, tid: library::TypeId, mut inner: String) -> Result {
    let volatile = inner.starts_with("volatile ");
    if volatile {
        inner = inner["volatile ".len()..].into();
    }

    let typ = env.library.type_(tid);
    let res = match *typ {
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
                UChar => "c_char",
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
                Pointer => match &inner[..]  {
                    "void" => "c_void",
                    _ => &*inner,
                },
                Unsupported => return Err(format!("[Unsupported type {}]", inner)),
                VarArgs => panic!("Should not reach here"),
            };
            Ok(inner.into())
        }
        Type::Record(..) | Type::Alias(..) | Type::Function(..) => {
            if let Some(declared_c_type) = typ.get_glib_name() {
                if declared_c_type != inner {
                    let msg = format!("[c:type mismatch `{}` != `{}` of `{}`]",
                                      inner, declared_c_type, typ.get_name());
                    warn!("{}", msg);
                    return Err(msg);
                }
            }
            else {
                warn!("Type `{}` missing c_type", typ.get_name());
            }
            fix_name(env, tid, &inner)
        }
        Type::CArray(inner_tid) => ffi_inner(env, inner_tid, inner),
        Type::FixedArray(inner_tid, size) => {
            match ffi_inner(env, inner_tid, inner) {
                Ok(s) => Ok(format!("[{}; {}]", s, size)),
                Err(s) => Err(format!("[{}; {}]", s, size)),
            }
        }
        Type::Array(..) | Type::PtrArray(..)
                | Type::List(..) | Type::SList(..) | Type::HashTable(..) => {
            fix_name(env, tid, &inner)
        }
        _ => {
            if let Some(glib_name) = env.library.type_(tid).get_glib_name() {
                if inner != glib_name && !implements_c_type(&env.library, tid, &inner) {
                    let msg = format!("[c:type mismatch {} != {} of {}]", inner, glib_name,
                      env.library.type_(tid).get_name());
                    warn!("{}", msg);
                    Err(msg)
                }
                else {
                    fix_name(env, tid, &inner)
                }
            }
            else {
                let msg = format!("[Missing glib_name of {}, can't match != {}]",
                                  env.library.type_(tid).get_name(), inner);
                warn!("{}", msg);
                Err(msg)
            }
        }
    };

    if volatile {
        res.map(|s| format!("Volatile<{}>", s))
    }
    else {
        res
    }
}

fn fix_name(env: &Env, type_id: library::TypeId, name: &str) -> Result {
    if type_id.ns_id == library::INTERNAL_NAMESPACE {
        match *env.library.type_(type_id) {
            Type::Array(..) | Type::PtrArray(..)
                    | Type::List(..) | Type::SList(..) | Type::HashTable(..) => {
                if Some(MAIN_NAMESPACE) == env.library.find_namespace("GLib") {
                    Ok(name.into())
                }
                else {
                    Ok(format!("{}_ffi::{}", crate_name("GLib"), name))
                }
            }
            _ => Ok(name.into())
        }
    } else {
        let name_with_prefix = if type_id.ns_id == library::MAIN_NAMESPACE {
            name.into()
        } else {
            format!("{}_ffi::{}", fix_namespace(env, type_id), name)
        };
        if env.type_status_sys(&type_id.full_name(&env.library)).ignored() {
            Err(name_with_prefix)
        } else {
            Ok(name_with_prefix)
        }
    }
}

//TODO: check if need to use in non sys codegen
fn fix_namespace(env: &Env, type_id: library::TypeId) -> String {
    crate_name(&env.library.namespace(type_id.ns_id).name)
}

fn implements_c_type(library: &library::Library, tid: TypeId, c_type: &str) -> bool {
    if let Some(ref klass) = library.type_(tid).maybe_ref_as::<library::Class>() {
        klass.implements.iter().chain(klass.parents.iter())
            .any(|&super_tid| library.type_(super_tid).get_glib_name() == Some(c_type))
    }
    else if let Some(ref iface) = library.type_(tid).maybe_ref_as::<library::Interface>() {
        iface.prereq_parents.iter()
            .any(|&super_tid| library.type_(super_tid).get_glib_name() == Some(c_type))
    }
    else {
        false
    }
}

fn rustify_pointers(c_type: &str) -> (String, String) {
    let mut input = c_type.trim();
    let leading_const = input.starts_with("const ");
    if leading_const {
        input = &input[6..];
    }
    let end = [
        input.find(" const"),
        input.find("*const"),
        input.find("*"),
        Some(input.len()),
    ].iter().filter_map(|&x| x).min().unwrap();
    let inner = input[..end].trim().into();

    let mut ptrs: Vec<_> = input[end..].rsplit('*').skip(1)
        .map(|s| if s.contains("const") { "*const" } else { "*mut" }).collect();
    if let (true, Some(p)) = (leading_const, ptrs.last_mut()) {
        *p = "*const";
    }

    let res = (ptrs.connect(" "), inner);
    trace!("rustify `{}` -> `{}` `{}`", c_type, res.0, res.1);
    res
}

#[cfg(test)]
mod tests {
    use super::rustify_pointers as rustify_ptr;

    fn s(x: &str, y: &str) -> (String, String) {
        (x.into(), y.into())
    }

    #[test]
    fn rustify_pointers() {
        assert_eq!(rustify_ptr("char"), s("", "char"));
        assert_eq!(rustify_ptr("char*"), s("*mut", "char"));
        assert_eq!(rustify_ptr("const char*"), s("*const", "char"));
        assert_eq!(rustify_ptr("char const*"), s("*const", "char"));
        assert_eq!(rustify_ptr("char const *"), s("*const", "char"));
        assert_eq!(rustify_ptr(" char * * "), s("*mut *mut", "char"));
        assert_eq!(rustify_ptr("const char**"), s("*mut *const", "char"));
        assert_eq!(rustify_ptr("char const**"), s("*mut *const", "char"));
        assert_eq!(rustify_ptr("const char* const*"), s("*const *const", "char"));
        assert_eq!(rustify_ptr("char const * const *"), s("*const *const", "char"));
        assert_eq!(rustify_ptr("char* const*"), s("*const *mut", "char"));
    }
}
