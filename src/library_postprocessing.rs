use library::*;

impl Namespace {
    fn unresolved(&self) -> Vec<&str> {
        self.index.iter().filter_map(|(name, &id)| {
            if self.types[id as usize].is_none() {
                Some(&name[..])
            } else {
                None
            }
        }).collect()
    }
}

impl Library {
    pub fn postprocessing(&mut self) {
        self.fix_gtype();
        self.check_resolved();
    }

    fn fix_gtype(&mut self) {
        if let Some(ns_id) = self.find_namespace("GObject") {
            // hide the `GType` type alias in `GObject`
            self.add_type(ns_id, "Type", Type::Fundamental(Fundamental::Unsupported));
        }
    }

    fn check_resolved(&self) {
        let list: Vec<_> = self.index.iter().flat_map(|(name, &id)| {
            let name = name.clone();
            self.namespace(id).unresolved().into_iter().map(move |s| format!("{}.{}", name, s))
        }).collect();

        if !list.is_empty() {
            panic!("Incomplete library, unresolved: {:?}", list);
        }
    }
}
