
    fn local_command_line(&self, application: &T, arguments: &mut ArgumentList) -> Option<i32> {
        application.parent_local_command_line(arguments)
    }
