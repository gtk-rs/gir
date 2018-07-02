use analysis::conversion_type::ConversionType;
use analysis::virtual_methods;
use analysis::object;
use traits::IntoString;
use library;
use env;

pub trait TrampolineToGlib {
    fn trampoline_to_glib(&self, env: &env::Env) -> String;
    fn trampoline_to_glib_as_function(&self, env: &env::Env, object: Option<&object::Info>, method: Option<&virtual_methods::Info>) -> (String, String);
}

impl TrampolineToGlib for library::Parameter {
    fn trampoline_to_glib(&self, env: &env::Env) -> String {
        use analysis::conversion_type::ConversionType::*;
        match ConversionType::of(env, self.typ) {
            Direct => String::new(),
            Scalar => ".to_glib()".to_owned(),
            Pointer => to_glib_xxx(self.transfer).to_owned(),
            Borrow => "/*Not applicable conversion Borrow*/".to_owned(),
            Unknown => "/*Unknown conversion*/".to_owned(),
        }
    }

    fn trampoline_to_glib_as_function(&self, env: &env::Env, object: Option<&object::Info>, method: Option<&virtual_methods::Info>) -> (String, String){
        use analysis::conversion_type::ConversionType::*;
        use codegen::sys::ffi_type::ffi_type;
        use analysis::rust_type::rust_type;

        // TODO: handle out parameters
        match ConversionType::of(env, self.typ) {
            Direct => (String::new(), String::new()),
            Scalar => (String::new(), ".to_glib()".to_owned()),
            Pointer => {
                if *self.nullable{
                    // FIXME: isn't there any other way to know if we need to return a mutable ptr?
                    let mut_str = if self.c_type.starts_with("const ") {""} else {"_mut"};
                    let left = "match ".to_owned();
                    let right = (if self.transfer == library::Transfer::None {
                        format!(r#"
    {{
        Some(t) => {{
            let ret = t{to_glib};
            unsafe extern "C" fn destroy_{ret_param}(p: glib_ffi::gpointer){{
                {rust_type}::from_glib_full(p as {c_type});
            }};
            gobject_ffi::g_object_set_qdata_full(gptr as *mut gobject_ffi::GObject,
                glib_ffi::g_quark_from_string("rs_{object_name}_{method_name}".to_glib_none().0),
                ret as *mut c_void,
                Some(destroy_{ret_param})
            );
            ret
        }},
        None => ptr::null{mut_str}()
    }}"#,
                        object_name=object.map(|ref o| o.module_name(env).unwrap_or(o.name.to_lowercase())).unwrap_or("".to_string()),
                        method_name=method.map(|ref m| &m.name).unwrap_or(&"".to_string()),
                        to_glib=to_glib_xxx( if self.transfer == library::Transfer::None {
                            library::Transfer::Full
                        }else{
                            self.transfer
                        }),
                        mut_str=mut_str,
                        ret_param="ret",
                        rust_type=rust_type(env, self.typ).into_string(),
                        c_type= ffi_type(env, self.typ, &self.c_type).into_string())
                    } else{
                        format!("{{ Some(t) => t{}, None => ptr::null{}()}}", to_glib_xxx(self.transfer), mut_str)
                    }).to_owned();

                    (left, right)
                }else{
                    (String::new(), to_glib_xxx(self.transfer).to_owned())
                }
            }
            Borrow => (String::new(), "/*Not applicable conversion Borrow*/".to_owned()),
            Unknown => (String::new(), "/*Unknown conversion*/".to_owned()),
        }
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
