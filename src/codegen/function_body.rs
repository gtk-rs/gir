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
    parameters: Vec<(String, bool)>,
    outs_as_return: bool,
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
    pub fn parameter(&mut self, parameter: String, out_as_return: bool) -> &mut Builder {
        self.parameters.push((parameter, out_as_return));
        if out_as_return { self.outs_as_return = true }
        self
    }
    pub fn generate(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::with_capacity(16);
        let unsafed = self.generate_unsafed();
        if self.outs_as_return {
            self.write_out_variables(&mut v);
        }
        write_to_vec!(v, "unsafe {{");
        write_to_vec!(v, "{}{}", tabs(1), unsafed);
        write_to_vec!(v, "}}");
        if self.outs_as_return {
            write_to_vec!(v, "{}", self.generate_out_return());
        }
        v
    }
    fn generate_unsafed(&self) -> String {
        let param_str = self.generate_func_parameters();
        format!("{}ffi::{}({}){}", self.from_glib_prefix,
            self.glib_name, param_str, self.from_glib_suffix)
    }
    fn generate_func_parameters(&self) -> String {
        let mut param_str = String::with_capacity(100);
        for (pos, &(ref par, out_as_returns)) in self.parameters.iter().enumerate() {
            if pos > 0 { param_str.push_str(", ") }
            if out_as_returns { param_str.push_str("&") }
            param_str.push_str(&*par);
        }
        param_str
    }
    fn get_outs(&self) -> Vec<&str> {
        self.parameters.iter()
            .filter_map(|&(ref s, out_as_returns)|
                if out_as_returns { Some(&s[..]) } else { None })
            .collect()
    }
    fn write_out_variables(&self, v: &mut Vec<String>) {
        let outs = self.get_outs();
        for par in outs {
            write_to_vec!(v, "let {} = Default::default();", par);
        }
    }
    fn generate_out_return(&self) -> String {
        let outs = self.get_outs();
        let (prefix, suffix) = if outs.len() > 1 { ("(", ")") } else { ("", "") };
        let mut ret_str = String::with_capacity(100);
        ret_str.push_str(prefix);
        for (pos, par) in outs.iter().enumerate() {
            if pos > 0 { ret_str.push_str(", ") }
            ret_str.push_str(par);
        }
        ret_str.push_str(suffix);
        ret_str
    }
}
