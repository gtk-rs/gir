use library;

pub trait TranslateToGlib {
    fn translate_to_glib(&self, library: &library::Library, in_trait: bool) -> String;
}

impl TranslateToGlib for library::Parameter {
    fn translate_to_glib(&self, _library: &library::Library, in_trait: bool) -> String {
        if self.instance_parameter {
            let upcast_str = if in_trait { ".upcast()" } else { "" };
            format!("self{}.to_glib_none().0", upcast_str)
        } else {
            format!("TODO:{}", self.name)
        }
    }
}
