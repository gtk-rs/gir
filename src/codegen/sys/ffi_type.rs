use analysis::rust_type::Result;
use env::Env;
use gobjects::GStatus;
use library;
use library::*;
use nameutil::module_name;

// FIXME: This module needs redundant allocations audit
// TODO: ffi_type computations should be cached

pub fn ffi_type(env: &Env, tid: library::TypeId, c_type: &str) -> Result {
    let (ptr, mut inner) = rustify_pointers(c_type);
    match *env.library.type_(tid) {
        Type::Fundamental(fund) => {
            use library::Fundamental::*;
            inner = match fund {
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
                Pointer => &*inner,
                Unsupported => return Err(format!("Unsupported type {}", c_type)),
                VarArgs => panic!("Should not reach here"),
            }.into();
        }
        Type::Record(..)
            | Type::Alias(..)
            | Type::Callback(..) => return Err(format!("Not yet supported type {}", c_type)),
        // TODO: need to recurse into it
        Type::Array(..) => return Err(format!("Not yet supported type {}", c_type)),
        Type::List(..) | Type::SList(..) => (),
        _ => {
            if let Some(glib_name) = env.library.type_(tid).get_glib_name() {
                if inner != glib_name {
                    let msg = format!("c:type mismatch {} != {} of {}", inner, glib_name,
                      env.library.type_(tid).get_name());
                warn!("{}", msg);
                return Err(msg);
                }
            }
            else {
                warn!("Missing glib_name of {}, can't match != {}",
                      env.library.type_(tid).get_name(), inner);
                return Err(format!("Missing glib_name of {}, can't match != {}",
                                    env.library.type_(tid).get_name(), inner));
            }
            inner = try!(fix_external_name(env, tid, &inner));
        }
    }
    let res = if ptr.is_empty() { inner } else { [ptr, inner].connect(" ") };
    trace!("ffi_type({:?}, {}) -> {}", tid, c_type, res);
    Ok(res)
}

fn fix_external_name(env: &Env, type_id: library::TypeId, name: &str) -> Result {
    if type_id.ns_id == library::MAIN_NAMESPACE || type_id.ns_id == library::INTERNAL_NAMESPACE {
        Ok(name.into())
    } else {
        let name_with_prefix = format!("{}_ffi::{}",
            fix_namespace(env, type_id), name);
        if env.type_status(&type_id.full_name(&env.library)) == GStatus::Ignore {
            Err(name_with_prefix.into())
        } else {
            Ok(name_with_prefix)
        }
    }
}

//TODO: check if need to use in non sys codegen
fn fix_namespace(env: &Env, type_id: library::TypeId) -> String {
    let mut name: &str = &module_name(&env.library.namespace(type_id.ns_id).name);
    name = match name {
        "gdk_pixbuf" => "gdk",
        "gio" => "glib",
        "gobject" => "glib",
        _ => name,
    };
    name.into()
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

    (ptrs.connect(" "), inner)
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
