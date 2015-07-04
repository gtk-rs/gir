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
    from_glib_prefix: String,
    from_glib_suffix: String,
}

impl Builder {
    pub fn new() -> Builder {
        Default::default()
    }
    pub fn glib_name(&mut self, name: &str) -> &mut Builder {
        self.glib_name = name.into();
        self
    }
    pub fn from_glib(&mut self, prefix_suffix: (String, String)) -> &mut Builder {
        self.from_glib_prefix = prefix_suffix.0;
        self.from_glib_suffix = prefix_suffix.1;
        self
    }
    pub fn generate(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        let unsafed = self.generate_unsafed();
        write_to_vec!(v, "unsafe {{");
        write_to_vec!(v, "{}{}", tabs(1), unsafed);
        write_to_vec!(v, "}}");
        v
    }
    fn generate_unsafed(&self) -> String {
        format!("{}ffi::{}(TODO:params){}", self.from_glib_prefix,
            self.glib_name, self.from_glib_suffix)
    }
}
