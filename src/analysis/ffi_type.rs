use log::{info, trace};

use crate::{
    analysis::{
        c_type::{implements_c_type, rustify_pointers},
        is_gpointer,
        rust_type::{Result, TypeError},
    },
    env::Env,
    library::*,
    nameutil::{use_glib_if_needed, use_glib_type},
    traits::*,
};

pub fn used_ffi_type(env: &Env, type_id: TypeId, c_type: &str) -> Option<String> {
    let (_ptr, inner) = rustify_pointers(c_type);
    let type_ = ffi_inner(env, type_id, &inner);
    type_.ok().and_then(|type_name| {
        if type_name.as_str().find(':').is_some() {
            Some(type_name.into_string())
        } else {
            None
        }
    })
}

pub fn ffi_type(env: &Env, tid: TypeId, c_type: &str) -> Result {
    let (ptr, inner) = rustify_pointers(c_type);
    let res = if ptr.is_empty() {
        if let Some(c_tid) = env.library.find_type(0, c_type) {
            // Fast track plain basic types avoiding some checks
            if env.library.type_(c_tid).maybe_ref_as::<Basic>().is_some() {
                match env.library.type_(tid) {
                    Type::FixedArray(_, size, _) => {
                        ffi_inner(env, c_tid, c_type).map_any(|rust_type| {
                            rust_type.alter_type(|typ_| format!("[{typ_}; {size}]"))
                        })
                    }
                    Type::Class(Class {
                        c_type: expected, ..
                    })
                    | Type::Interface(Interface {
                        c_type: expected, ..
                    }) if is_gpointer(c_type) => {
                        info!("[c:type `gpointer` instead of `*mut {}`, fixing]", expected);
                        ffi_inner(env, tid, expected).map_any(|rust_type| {
                            rust_type.alter_type(|typ_| format!("*mut {typ_}"))
                        })
                    }
                    _ => ffi_inner(env, c_tid, c_type),
                }
            } else {
                // c_type isn't basic
                ffi_inner(env, tid, &inner)
            }
        } else {
            // c_type doesn't match any type in the library by name
            ffi_inner(env, tid, &inner)
        }
    } else {
        // ptr not empty
        ffi_inner(env, tid, &inner)
            .map_any(|rust_type| rust_type.alter_type(|typ_| format!("{ptr} {typ_}")))
    };
    trace!("ffi_type({:?}, {}) -> {:?}", tid, c_type, res);
    res
}

fn ffi_inner(env: &Env, tid: TypeId, inner: &str) -> Result {
    let typ = env.library.type_(tid);
    match *typ {
        Type::Basic(fund) => {
            use crate::library::Basic::*;
            let inner = match fund {
                None => "libc::c_void",
                Boolean => return Ok(use_glib_if_needed(env, "ffi::gboolean").into()),
                Int8 => "i8",
                UInt8 => "u8",
                Int16 => "i16",
                UInt16 => "u16",
                Int32 => "i32",
                UInt32 => "u32",
                Int64 => "i64",
                UInt64 => "u64",
                Char => "libc::c_char",
                UChar => "libc::c_uchar",
                Short => "libc::c_short",
                UShort => "libc::c_ushort",
                Int => "libc::c_int",
                UInt => "libc::c_uint",
                Long => "libc::c_long",
                ULong => "libc::c_ulong",
                Size => "libc::size_t",
                SSize => "libc::ssize_t",
                Float => "libc::c_float",
                Double => "libc::c_double",
                UniChar => "u32",
                Utf8 => "libc::c_char",
                Filename => "libc::c_char",
                Type => return Ok(use_glib_if_needed(env, "ffi::GType").into()),
                IntPtr => "libc::intptr_t",
                UIntPtr => "libc::uintptr_t",
                Bool => "bool",
                _ => return Err(TypeError::Unimplemented(inner.into())),
            };
            Ok(inner.into())
        }
        Type::Union(..) | Type::Record(..) | Type::Alias(..) | Type::Function(..) => {
            if let Some(declared_c_type) = typ.get_glib_name() {
                if declared_c_type != inner {
                    let msg = format!(
                        "[c:type mismatch `{}` != `{}` of `{}`]",
                        inner,
                        declared_c_type,
                        typ.get_name()
                    );
                    warn_main!(tid, "{}", msg);
                    return Err(TypeError::Mismatch(msg));
                }
            } else {
                warn_main!(tid, "Type `{}` missing c_type", typ.get_name());
            }
            fix_name(env, tid, inner)
        }
        Type::CArray(inner_tid) => ffi_inner(env, inner_tid, inner),
        Type::FixedArray(inner_tid, size, _) => ffi_inner(env, inner_tid, inner)
            .map_any(|rust_type| rust_type.alter_type(|typ_| format!("[{typ_}; {size}]"))),
        Type::Array(..)
        | Type::PtrArray(..)
        | Type::List(..)
        | Type::SList(..)
        | Type::HashTable(..) => fix_name(env, tid, inner),
        _ => {
            if let Some(glib_name) = env.library.type_(tid).get_glib_name() {
                if inner != glib_name {
                    if implements_c_type(env, tid, inner) {
                        info!(
                            "[c:type {} of {} <: {}, fixing]",
                            glib_name,
                            env.library.type_(tid).get_name(),
                            inner
                        );
                        fix_name(env, tid, glib_name)
                    } else {
                        let msg = format!(
                            "[c:type mismatch {} != {} of {}]",
                            inner,
                            glib_name,
                            env.library.type_(tid).get_name()
                        );
                        warn_main!(tid, "{}", msg);
                        Err(TypeError::Mismatch(msg))
                    }
                } else {
                    fix_name(env, tid, inner)
                }
            } else {
                let msg = format!(
                    "[Missing glib_name of {}, can't match != {}]",
                    env.library.type_(tid).get_name(),
                    inner
                );
                warn_main!(tid, "{}", msg);
                Err(TypeError::Mismatch(msg))
            }
        }
    }
}

fn fix_name(env: &Env, type_id: TypeId, name: &str) -> Result {
    if type_id.ns_id == INTERNAL_NAMESPACE {
        match env.library.type_(type_id) {
            Type::Array(..)
            | Type::PtrArray(..)
            | Type::List(..)
            | Type::SList(..)
            | Type::HashTable(..) => Ok(use_glib_if_needed(env, &format!("ffi::{name}")).into()),
            _ => Ok(name.into()),
        }
    } else {
        let sys_crate_name = &env.namespaces[type_id.ns_id].sys_crate_name;
        let sys_crate_name = if sys_crate_name == "gobject_ffi" {
            use_glib_type(env, "gobject_ffi")
        } else if type_id.ns_id == MAIN_NAMESPACE {
            sys_crate_name.clone()
        } else {
            format!(
                "{}::{}",
                env.namespaces[type_id.ns_id].crate_name, sys_crate_name
            )
        };
        let name_with_prefix = format!("{sys_crate_name}::{name}");
        if env
            .type_status_sys(&type_id.full_name(&env.library))
            .ignored()
        {
            Err(TypeError::Ignored(name_with_prefix))
        } else {
            Ok(name_with_prefix.into())
        }
    }
}
