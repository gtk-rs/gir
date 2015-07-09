use analysis::rust_type::Result;
use env::Env;
use gobjects::GStatus;
use library;
use nameutil::module_name;

pub fn ffi_type(env: &Env, type_id: library::TypeId) -> Result {
    use library::Type::*;
    use library::Fundamental::*;
    let type_ = env.library.type_(type_id);
    match type_ {
        &Fundamental(fund) => {
            let ok = |s: &str| Ok(s.into());
            let err = |s: &str| Err(s.into());
            match fund {
                None => err("()"),
                Boolean => ok("gboolean"),
                Int8 => ok("gint8"),
                UInt8 => ok("guint8"),
                Int16 => ok("gint16"),
                UInt16 => ok("guint16"),
                Int32 => ok("gint32"),
                UInt32 => ok("guint32"),
                Int64 => ok("gint64"),
                UInt64 => ok("guint64"),

                Int => ok("gint"),      //maybe dependent on target system
                UInt => ok("guint"),     //maybe dependent on target system

                Float => ok("gfloat"),
                Double => ok("gdouble"),

                Utf8 => ok("*const c_char"),

                Type => ok("GType"),
                Unsupported => err("Unsupported"),
                _ => err(&format!("Fundamental: {:?}", fund)),
            }
        },

        &Enumeration(ref enum_) => Ok(format!("enums::{}", enum_.name)),
        &Interface(ref interface) => to_mut_ptr(fix_external_name(env, type_id, &interface.glib_type_name)),
        &Class(ref klass) => to_mut_ptr(fix_external_name(env, type_id, &klass.glib_type_name)),
        _ => Err(format!("Unknown rust type: {:?}", type_.get_name() )),
        //TODO: check usage library::Type::get_name() when no _ in this
    }
}

fn fix_external_name(env: &Env, type_id: library::TypeId, name: &str) -> Result {
    if type_id.ns_id == library::MAIN_NAMESPACE || type_id.ns_id == library::INTERNAL_NAMESPACE {
        Ok(name.into())
    } else {
        let name_with_prefix = format!("{}_ffi::{}",
            module_name(&env.library.namespace(type_id.ns_id).name), name);
        if env.type_status(&type_id.full_name(&env.library)) == GStatus::Ignore {
            Err(name_with_prefix.into())
        } else {
            Ok(name_with_prefix)
        }
    }
}

fn to_mut_ptr(res: Result) -> Result {
    match res {
        Ok(s) => Ok(format!("*mut {}", s)),
        Err(s) => Err(format!("*mut {}", s)),
    }
}
