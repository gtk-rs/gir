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
    use analysis::rust_type::rust_type;
    use codegen::sys::ffi_type::ffi_type;

    let param_name = if parameter.name.len() > 0 { parameter.name.clone() } else {"ret".to_string()};

    // TODO: handle out parameters
    match ConversionType::of(env, parameter.typ) {
        Direct => format!("rs_{}{}", param_name, parameter.trampoline_to_glib(env)),
        Scalar => format!("rs_{}{}", param_name, parameter.trampoline_to_glib(env)),
        Pointer => {
            if *parameter.nullable {

                // FIXME: isn't there any other way to know if we need to return a mutable ptr?
                let mut_str = if parameter.c_type.starts_with("const ") {
                    ""
                } else {
                    "_mut"
                };
                let right = (if parameter.transfer == library::Transfer::None {
                    format!(r#"
    match rs_{param_name} {{
        Some(t_{param_name}) => {{
            let ret = t_{param_name}{to_glib};
            unsafe extern "C" fn destroy_{param_name}(p: glib_ffi::gpointer){{
                {rust_type}::from_glib_full(p as {c_type});
            }};
            gobject_ffi::g_object_set_qdata_full(gptr as *mut gobject_ffi::GObject,
                glib_ffi::g_quark_from_string("rs_{object_name}_{method_name}_{param_name}".to_glib_none().0),
                ret as *mut c_void,
                Some(destroy_{param_name})
            );
            ret
        }},
        None => ptr::null{mut_str}()
    }}"#,
                    object_name=object.module_name(env).unwrap_or(object.name.to_lowercase()),
                    method_name=method.name,
                    to_glib=to_glib_xxx( if parameter.transfer == library::Transfer::None {
                        library::Transfer::Full
                    }else{
                        parameter.transfer
                    }),
                    mut_str=mut_str,
                    param_name=param_name,
                    rust_type=rust_type(env, parameter.typ).into_string(),
                    c_type= ffi_type(env, parameter.typ, &parameter.c_type).into_string())
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
                format!("rs_{}{}", param_name, parameter.trampoline_to_glib(env))
            }
        }
        Borrow => format!("rs_{}{}", param_name, parameter.trampoline_to_glib(env)),
        Unknown => format!("rs_{}{}", param_name, parameter.trampoline_to_glib(env)),
    }
}

fn to_glib_xxx(transfer: library::Transfer) -> &'static str {
    use library::Transfer::*;
    match transfer {
        None => "/*Not checked*/.to_glib_none().0",
        Full => ".to_glib_full()",
        Container => "/*Not checked*/.to_glib_container().0",
    }
}
