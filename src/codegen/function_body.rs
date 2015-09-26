use std::fmt;

use super::general::tabs;

macro_rules! write_to_vec(
    ($dst:expr, $($arg:tt)*) => (
        $dst.push(fmt::format(format_args!($($arg)*)))
    )
);

#[derive(Clone, Debug)]
enum Parameter {
    In { parameter: String },
    Out {
        name: String,
        prefix: String,
        suffix: String
    },
}

use self::Parameter::*;

#[derive(Default, Debug)]
pub struct Builder {
    glib_name: String,
    from_glib_prefix: String,
    from_glib_suffix: String,
    parameters: Vec<Parameter>,
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
    pub fn parameter(&mut self, parameter: String) -> &mut Builder {
        self.parameters.push(Parameter::In { parameter: parameter });
        self
    }
    pub fn out_parameter(&mut self, name: String, prefix: String, suffix: String) -> &mut Builder {
        self.parameters.push(Parameter::Out{ name: name, prefix: prefix, suffix: suffix });
        self.outs_as_return = true;
        self
    }
    pub fn generate(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::with_capacity(16);
        let unsafed = self.generate_unsafed();
        write_to_vec!(v, "unsafe {{");
        if self.outs_as_return {
            self.write_out_variables(&mut v, 1);
        }
        write_to_vec!(v, "{}{}", tabs(1), unsafed);
        if self.outs_as_return {
            write_to_vec!(v, "{}{}", tabs(1), self.generate_out_return());
        }
        write_to_vec!(v, "}}");
        v
    }
    fn generate_unsafed(&self) -> String {
        let param_str = self.generate_func_parameters();
        format!("{}ffi::{}({}){}", self.from_glib_prefix,
            self.glib_name, param_str, self.from_glib_suffix)
    }
    fn generate_func_parameters(&self) -> String {
        let mut param_str = String::with_capacity(100);
        for (pos, par) in self.parameters.iter().enumerate() {
            if pos > 0 { param_str.push_str(", ") }
            match par {
                &In { ref parameter } => param_str.push_str(&*parameter),
                &Out { ref name, .. } => {
                    param_str.push_str("&mut ");
                    param_str.push_str(&*name);
                },
            }
        }
        param_str
    }
    fn get_outs(&self) -> Vec<&Parameter> {
        self.parameters.iter()
            .filter_map(|par| if let Out{ .. } = *par { Some(par) } else { None })
            .collect()
    }
    fn write_out_variables(&self, v: &mut Vec<String>, indent: i32) {
        let outs = self.get_outs();
        for par in outs {
            if let Out{ ref name, .. } = *par {
                write_to_vec!(v, "{}let mut {} = mem::uninitialized();", tabs(indent), name);
            }
        }
    }
    fn generate_out_return(&self) -> String {
        let outs = self.get_outs();
        let (prefix, suffix) = if outs.len() > 1 { ("(", ")") } else { ("", "") };
        let mut ret_str = String::with_capacity(100);
        ret_str.push_str(prefix);
        for (pos, par) in outs.iter().enumerate() {
            if let Out{ ref name, ref prefix, ref suffix } = **par {
                if pos > 0 { ret_str.push_str(", ") }
                ret_str.push_str(&*prefix);
                ret_str.push_str(&*name);
                ret_str.push_str(&*suffix);
            }
        }
        ret_str.push_str(suffix);
        ret_str
    }
}
