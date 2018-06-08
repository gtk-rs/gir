
            fn local_command_line(&self, application: &T, arguments: &mut ArgumentList) -> Option<i32>{
                let imp: &$name<T> = self.as_ref();
                imp.local_command_line(application, arguments)
            }
