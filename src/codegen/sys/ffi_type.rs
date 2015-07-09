use env::Env;
use library;
use analysis::rust_type::Result;

pub fn ffi_type(env: &Env, type_id: library::TypeId) -> Result {
    use library::Type::*;
    use library::Fundamental::*;
    let type_ = env.library.type_(type_id);
    let ffi_type = match type_ {
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

        &Enumeration(ref enum_) => Ok(enum_.glib_type_name.clone()),
        &Interface(ref interface) => Ok(format!("*mut {}", interface.glib_type_name)),
        &Class(ref klass) => Ok(format!("*mut {}", klass.glib_type_name)),
        _ => Err(format!("Unknown rust type: {:?}", type_.get_name() )),
        //TODO: check usage library::Type::get_name() when no _ in this
    };
    ffi_type
}
