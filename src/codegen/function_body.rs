use std::fmt;

use super::general::tabs;

macro_rules! write_to_vec(
    ($dst:expr, $($arg:tt)*) => (
        $dst.push(fmt::format(format_args!($($arg)*)))
    )
);

#[derive(Default, Debug)]
pub struct Builder {
    glib_name: String,
}

impl Builder {
    pub fn new() -> Builder {
        Default::default()
    }
    pub fn glib_name(&mut self, name: &str) -> &mut Builder {
        self.glib_name = name.into();
        self
    }
    pub fn generate(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        //TODO: real generation
        write_to_vec!(v, "unsafe {{");
        write_to_vec!(v, "{}TODO: call ffi:{}()", tabs(1), self.glib_name);
        write_to_vec!(v, "}}");
        v
    }
}
