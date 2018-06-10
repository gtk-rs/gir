use analysis::conversion_type::ConversionType;
use library;
use env;

pub trait TrampolineToGlib {
    fn trampoline_to_glib(&self, env: &env::Env) -> String;
    fn trampoline_to_glib_as_function(&self, env: &env::Env) -> (String, String);
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

    fn trampoline_to_glib_as_function(&self, env: &env::Env) -> (String, String){

        use analysis::conversion_type::ConversionType::*;
        match ConversionType::of(env, self.typ) {
            Direct => (String::new(), String::new()),
            Scalar => (String::new(), ".to_glib()".to_owned()),
            Pointer => {
                if *self.nullable{
                    let left = "match ".to_owned();
                    let right = format!("{{ Some(t)  => t{}, None => std::ptr::null()}}", to_glib_xxx(self.transfer)).to_owned();

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
