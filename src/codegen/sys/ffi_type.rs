use analysis::c_type::rustify_pointers;
use analysis::namespaces;
use analysis::rust_type::{Result, TypeError};
use env::Env;
use library;
use library::*;
use traits::*;

// FIXME: This module needs redundant allocations audit
// TODO: ffi_type computations should be cached

pub fn ffi_type(env: &Env, tid: library::TypeId, c_type: &str) -> Result<'static> {
    let (ptr, inner) = rustify_pointers(c_type);
    let res = if ptr.is_empty() {
        if let Some(c_tid) = env.library.find_type(0, c_type) {
            // Fast track plain fundamental types avoiding some checks
            if env.library.type_(c_tid).maybe_ref_as::<Fundamental>().is_some() {
                match *env.library.type_(tid) {
                    Type::FixedArray(_, size) => {
                        ffi_inner(env, c_tid, c_type.into())
                            .map_any(|s| format!("[{}; {}]", s, size).into())
                    }
                    Type::Class(Class { c_type: ref expected, .. }) |
                            Type::Interface(Interface { c_type: ref expected, .. })
                            if c_type == "gpointer" => {
                        info!("[c:type `gpointer` instead of `*mut {}`, fixing]", expected);
                        ffi_inner(env, tid, expected.clone())
                            .map_any(|s| format!("*mut {}", s).into())
                    }
                    _ => {
                        ffi_inner(env, c_tid, c_type.into())
                    }
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
            .map_any(|s| format!("{} {}", ptr, s).into())
    };
    trace!("ffi_type({:?}, {}) -> {:?}", tid, c_type, res);
    res
}

fn ffi_inner(env: &Env, tid: library::TypeId, mut inner: String) -> Result<'static> {
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
                Pointer => match &inner[..]  {
                    "void" => "c_void",
                    "tm" => return Err(TypeError::Unimplemented(inner.into())),  //TODO: try use time:Tm
                    _ => return Ok(inner.into()),
                },
                Unsupported => return Err(TypeError::Unimplemented(inner.into())),
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
                    return Err(TypeError::Mismatch(msg.into()));
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
                .map_any(|s| format!("[{}; {}]", s, size).into())
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
                        Err(TypeError::Mismatch(msg.into()))
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
                Err(TypeError::Mismatch(msg.into()))
            }
        }
    };

    if volatile {
        res.map(|s| format!("Volatile<{}>", s).into())
    }
    else {
        res
    }
}

fn fix_name<'a>(env: &Env, type_id: library::TypeId, name: &'a str) -> Result<'static> {
    if type_id.ns_id == library::INTERNAL_NAMESPACE {
        match *env.library.type_(type_id) {
            Type::Array(..) | Type::PtrArray(..)
                    | Type::List(..) | Type::SList(..) | Type::HashTable(..) => {
                if env.namespaces.glib_ns_id == namespaces::MAIN {
                    Ok(Cow::Owned(name.into()))
                }
                else {
                    Ok(Cow::Owned(format!("{}::{}", &env.namespaces[env.namespaces.glib_ns_id].crate_name,
                        name)))
                }
            }
            _ => Ok(Cow::Owned(name.into()))
        }
    } else {
        let name_with_prefix = if type_id.ns_id == library::MAIN_NAMESPACE {
            Cow::Owned(name.into())
        } else {
            format!("{}::{}", &env.namespaces[type_id.ns_id].crate_name, name).into()
        };
        if env.type_status_sys(&type_id.full_name(&env.library)).ignored() {
            Err(TypeError::Ignored(name_with_prefix))
        } else {
            Ok(name_with_prefix)
        }
    }
}

fn implements_c_type(env: &Env, tid: TypeId, c_type: &str) -> bool {
    env.class_hierarchy.supertypes(tid).iter()
        .any(|&super_tid| env.library.type_(super_tid).get_glib_name() == Some(c_type))
}
