
    fn parent_command_line(&self, cmd_line: &gio::ApplicationCommandLine) -> i32 {
        unsafe {
            let klass = self.get_class();
            let parent_klass = (*klass).get_parent_class() as *const gio_ffi::GApplicationClass;
            (*parent_klass)
                .command_line
                .map(|f| f(self.to_glib_none().0, cmd_line.to_glib_none().0))
                .unwrap_or(0)
        }
    }
