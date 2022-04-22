use log::error;

pub trait TomlHelper
where
    Self: Sized,
{
    fn check_unwanted(&self, options: &[&str], err_msg: &str);
    fn lookup<'a>(&'a self, option: &str) -> Option<&'a toml::Value>;
    fn lookup_str<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a str, String>;
    fn lookup_vec<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a Vec<Self>, String>;
    fn as_result_str<'a>(&'a self, option: &'a str) -> Result<&'a str, String>;
    fn as_result_vec<'a>(&'a self, option: &'a str) -> Result<&'a Vec<Self>, String>;
    fn as_result_bool<'a>(&'a self, option: &'a str) -> Result<bool, String>;
}

impl TomlHelper for toml::Value {
    fn check_unwanted(&self, options: &[&str], err_msg: &str) {
        let mut ret = Vec::new();
        let table = match self.as_table() {
            Some(table) => table,
            None => return,
        };
        for (key, _) in table.iter() {
            if !options.contains(&key.as_str()) {
                ret.push(key.clone());
            }
        }
        if !ret.is_empty() {
            error!(
                "\"{}\": Unknown key{}: {:?}",
                err_msg,
                if ret.len() > 1 { "s" } else { "" },
                ret
            );
        }
    }
    fn lookup<'a>(&'a self, option: &str) -> Option<&'a toml::Value> {
        let mut value = self;
        for opt in option.split('.') {
            let table = value.as_table()?;
            value = table.get(opt)?;
        }
        Some(value)
    }
    fn lookup_str<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a str, String> {
        let value = self.lookup(option).ok_or(err)?;
        value.as_result_str(option)
    }
    fn lookup_vec<'a>(&'a self, option: &'a str, err: &str) -> Result<&'a Vec<Self>, String> {
        let value = self.lookup(option).ok_or(err)?;
        value.as_result_vec(option)
    }
    fn as_result_str<'a>(&'a self, option: &'a str) -> Result<&'a str, String> {
        self.as_str().ok_or_else(|| {
            format!(
                "Invalid `{}` value, expected a string, found {}",
                option,
                self.type_str()
            )
        })
    }
    fn as_result_vec<'a>(&'a self, option: &'a str) -> Result<&'a Vec<Self>, String> {
        self.as_array().ok_or_else(|| {
            format!(
                "Invalid `{}` value, expected a array, found {}",
                option,
                self.type_str()
            )
        })
    }
    fn as_result_bool<'a>(&'a self, option: &'a str) -> Result<bool, String> {
        self.as_bool().ok_or_else(|| {
            format!(
                "Invalid `{}` value, expected a boolean, found {}",
                option,
                self.type_str()
            )
        })
    }
}
