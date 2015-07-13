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
    let mut ok = true;
    match *env.library.type_(tid) {
        Type::Fundamental(fund) => {
            use library::Fundamental::*;
            inner = match fund {
                None => "c_void".into(),
                Boolean => "gboolean".into(),
                Int8 => "i8".into(),
                UInt8 => "u8".into(),
                Int16 => "i16".into(),
                UInt16 => "u16".into(),
                Int32 => "i32".into(),
                UInt32 => "u32".into(),
                Int64 => "i64".into(),
                UInt64 => "u64".into(),
                Char => "c_char".into(),
                UChar => "c_char".into(),
                Short => "c_short".into(),
                UShort => "c_ushort".into(),
                Int => "c_int".into(),
                UInt => "c_uint".into(),
                Long => "c_long".into(),
                ULong => "c_ulong".into(),
                Size => "size_t".into(),
                SSize => "ssize_t".into(),
                Float => "c_float".into(),
                Double => "c_double".into(),
                UniChar => "u32".into(),
                Utf8 => "c_char".into(),
                Filename => "c_char".into(),
                Type => "GType".into(),
                Pointer => inner,
                Unsupported => {
                    ok = false;
                    format!("[Unsupported type {}]", c_type)
                }
                VarArgs => panic!("Should not reach here"),
            };
        }
        Type::Record(..)
            | Type::Alias(..)
            | Type::Function(..) => {
                inner = format!("[Not yet supported type {}]", c_type);
                ok = false;
            }
        // TODO: need to recurse into it
        Type::Array(..) => {
            inner = format!("[Not yet supported type {}]", c_type);
            ok = false;
        }
        Type::List(..) | Type::SList(..) => (),
        _ => {
            if let Some(glib_name) = env.library.type_(tid).get_glib_name() {
                if inner != glib_name {
                    inner = format!("[c:type mismatch {} != {} of {}]", inner, glib_name,
                      env.library.type_(tid).get_name());
                    warn!("{}", inner);
                    ok = false;
                }
            }
            else {
                inner = format!("[Missing glib_name of {}, can't match != {}]",
                                  env.library.type_(tid).get_name(), inner);
                warn!("{}", inner);
                ok = false;
            }

            if ok {
                let fixed = fix_external_name(env, tid, &inner);
                ok = fixed.is_ok();
                inner = fixed.unwrap_or_else(|s| s);
            }
        }
    }
    let res = if ptr.is_empty() { inner } else { [ptr, inner].connect(" ") };
    trace!("ffi_type({:?}, {}) -> {}", tid, c_type, res);
    if ok { Ok(res) } else { Err(res) }
}

fn fix_external_name(env: &Env, type_id: library::TypeId, name: &str) -> Result {
    if type_id.ns_id == library::MAIN_NAMESPACE || type_id.ns_id == library::INTERNAL_NAMESPACE {
        Ok(name.into())
    } else {
        let name_with_prefix = format!("{}_ffi::{}",
            fix_namespace(env, type_id), name);
        if env.type_status_sys(&type_id.full_name(&env.library)) == GStatus::Ignore {
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
        "g_object" => "glib",
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
