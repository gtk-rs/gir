use crate::{config::WorkMode, library::*};

impl Library {
    pub fn preprocessing(&mut self, work_mode: WorkMode) {
        self.add_glib_priority(work_mode);
    }
}
