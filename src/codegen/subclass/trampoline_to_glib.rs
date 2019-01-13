use analysis::conversion_type::ConversionType;
use analysis::object;
use analysis::virtual_methods;
use codegen::trampoline_to_glib::TrampolineToGlib;
use env;
use library;
use traits::IntoString;

pub fn trampoline_to_glib(
    parameter: &library::Parameter,
    env: &env::Env,
    object: &object::Info,
    method: &virtual_methods::Info,
) -> String {
    use analysis::conversion_type::ConversionType::*;

    let param_name = if parameter.name.len() > 0 { parameter.name.clone() } else {"ret".to_string()};

    match ConversionType::of(env, parameter.typ) {
        Direct => format!("rs_{}{}", param_name, parameter.trampoline_to_glib(env)),
        Scalar => format!("rs_{}{}", param_name, parameter.trampoline_to_glib(env)),
        Pointer => {
            let type_ = env.library.type_(parameter.typ);
            let is_array_type = match type_ {
                library::Type::Array(..) |
                library::Type::CArray(..) |
                library::Type::PtrArray(..) |
                library::Type::List(..) |
                library::Type::SList(..) |
                library::Type::FixedArray(..) |
                library::Type::HashTable(..) => true,
                _ => false
            };
            if *parameter.nullable && !is_array_type{

                // FIXME: isn't there any other way to know if we need to return a mutable ptr?
                let mut_str = if parameter.c_type.starts_with("const ") {
                    ""
                } else {
                    "_mut"
                };
                let right = (if parameter.transfer == library::Transfer::None {
                    format!(r#"
    match rs_{param_name} {{
        Some(t_{param_name}) => {to_glib_destroy},
        None => ptr::null{mut_str}()
    }}"#,
                    to_glib_destroy=to_glib_with_destroy(parameter, env, object, method, &param_name, &"t_".to_string()),
                    mut_str=mut_str,
                    param_name=param_name)
                } else {
                    format!(
                        "match rs_{param_name} {{ Some(t_{param_name}) => t_{param_name}{to_glib}, None => ptr::null{mut_str}()}}",
                        param_name=param_name,
                        to_glib=parameter.trampoline_to_glib(env),
                        mut_str=mut_str
                    )
                }).to_owned();

                right
            } else {
                to_glib_with_destroy(parameter, env, object, method, &param_name, &"rs_".to_string()).to_owned()
            }
        }
        Borrow => format!("rs_{}{}", param_name, parameter.trampoline_to_glib(env)),
        Unknown => format!("rs_{}{}", param_name, parameter.trampoline_to_glib(env)),
    }
}

fn to_glib_with_destroy(parameter: &library::Parameter,
                        env: &env::Env,
                        object: &object::Info,
                        method: &virtual_methods::Info,
                        param_name: &String,
                        param_prefix: &String) -> String {
    use analysis::rust_type::rust_type;
    use codegen::sys::ffi_type::ffi_type;

    let type_ = &env.library.type_(parameter.typ);
    // TODO: way too ugly
    let c_type = type_.get_glib_name().map(|c| format!("{}*", c)).unwrap_or(parameter.c_type.clone());

    let is_container_type = match type_ {
        library::Type::Array(..) |
        library::Type::CArray(..) |
        library::Type::PtrArray(..) |
        library::Type::List(..) |
        library::Type::SList(..) |
        library::Type::FixedArray(..) => true,
        _ => false
    };


    let rust_type = rust_type(env, parameter.typ).into_string();

    format!(r#"{{
        let ret = {param_prefix}{param_name}{to_glib};
        unsafe extern "C" fn destroy_{param_name}(p: glib_ffi::gpointer){{
            let _: {rust_type} = {glib_translate}::from_glib_full(p as {c_type});
        }};
        gobject_ffi::g_object_set_qdata_full(gptr as *mut gobject_ffi::GObject,
            glib_ffi::g_quark_from_string("rs_{object_name}_{method_name}_{param_name}".to_glib_none().0),
            ret as *mut c_void,
            Some(destroy_{param_name})
        );
        ret
    }}"#,
    object_name=object.module_name(env).unwrap_or(object.name.to_lowercase()),
    method_name=method.name,
    to_glib=to_glib_xxx( if parameter.transfer == library::Transfer::None {
        library::Transfer::Full
    }else{
        parameter.transfer
    }),
    param_name=param_name,
    param_prefix=param_prefix,
    glib_translate= if is_container_type {"FromGlibPtrContainer".to_string()} else {rust_type.clone()},
    rust_type=rust_type,
    c_type=ffi_type(env, parameter.typ, &c_type).into_string())
}

fn to_glib_xxx(transfer: library::Transfer) -> &'static str {
    use library::Transfer::*;
    match transfer {
        None => "/*Not checked*/.to_glib_none().0",
        Full => ".to_glib_full()",
        Container => "/*Not checked*/.to_glib_container().0",
    }
}
