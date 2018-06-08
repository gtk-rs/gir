
    fn parent_local_command_line(&self, arguments: &mut ArgumentList) -> Option<i32> {
        unsafe {
            let klass = self.get_class();
            let parent_klass = (*klass).get_parent_class() as *const gio_ffi::GApplicationClass;
            let mut exit_status = 0;
            let success = (*parent_klass)
                .local_command_line
                .map(|f| {
                    let ret = f(self.to_glib_none().0, arguments.ptr, &mut exit_status);
                    arguments.refresh();
                    ret
                })
                .unwrap_or(glib_ffi::GFALSE);

            match success {
                glib_ffi::GTRUE => Some(exit_status),
                _ => None,
            }
        }
    }
