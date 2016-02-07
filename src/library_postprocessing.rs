use library::*;
use traits::MaybeRefAs;

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
        self.fill_class_relationships();
        self.fill_class_iface_relationships();
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

    fn fill_class_relationships(&mut self) {
        let mut classes = Vec::new();
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for (id, type_) in ns.types.iter().enumerate() {
                let tid = TypeId { ns_id: ns_id as u16, id: id as u32 };
                if let Type::Class(_) = *type_.as_ref().unwrap() {
                    classes.push(tid);
                }
            }
        }

        let mut parents = Vec::with_capacity(10);
        for tid in classes {
            parents.clear();

            let mut first_parent_tid: Option<TypeId> = None;
            if let Type::Class(ref klass) = *self.type_(tid) {
                let mut parent = klass.parent;
                if let Some(parent_tid) = parent {
                    first_parent_tid = Some(parent_tid);
                }
                while let Some(parent_tid) = parent {
                    parents.push(parent_tid);
                    parent = self.type_(parent_tid).to_ref_as::<Class>().parent;
                }
            }

            if let Type::Class(ref mut klass) = *self.type_mut(tid) {
                parents.iter().map(|&tid| klass.parents.push(tid)).count();
            }

            if let Some(parent_tid) = first_parent_tid {
                if let Type::Class(ref mut klass) = *self.type_mut(parent_tid) {
                    klass.children.insert(tid);
                }
            }
        }
    }

    fn fill_class_iface_relationships(&mut self) {
        let mut ifaces = Vec::new();
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for (id, type_) in ns.types.iter().enumerate() {
                let tid = TypeId { ns_id: ns_id as u16, id: id as u32 };
                if let Type::Interface(_) = *type_.as_ref().unwrap() {
                    ifaces.push(tid);
                }
            }
        }

        fn get_iface_prereqs(vec: &mut Vec<TypeId>, library: &Library, iface: &Interface) {
            for &tid in &iface.prerequisites {
                vec.push(tid);
                match *library.type_(tid) {
                    Type::Class(ref p_class) => {
                        for &tid in &p_class.parents {
                            vec.push(tid);
                        }
                    }
                    Type::Interface(ref p_iface) => get_iface_prereqs(vec, library, p_iface),
                    _ => {}
                }
            }
        }

        let mut prereqs = Vec::with_capacity(10);
        for tid in ifaces {
            prereqs.clear();
            if let Type::Interface(ref iface) = *self.type_(tid) {
                get_iface_prereqs(&mut prereqs, self, iface);
            }
            prereqs.sort();
            prereqs.dedup();
            if let Type::Interface(ref mut iface) = *self.type_mut(tid) {
                prereqs.iter().map(|&tid| iface.prereq_parents.push(tid)).count();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use library::*;
    use traits::*;

    fn make_library() -> Library {
        let mut lib = Library::new("Gtk");
        let glib_ns_id = lib.add_namespace("GLib");
        let gtk_ns_id = lib.add_namespace("Gtk");
        let object_tid = lib.add_type(glib_ns_id, "Object".into(), Type::Class(
            Class {
                name: "Object".into(),
                c_type: "GObject".into(),
                glib_get_type: "g_object_get_type".into(),
                .. Class::default()
            }));
        let ioobject_tid = lib.add_type(glib_ns_id, "InitiallyUnowned".into(), Type::Class(
            Class {
                name: "InitiallyUnowned".into(),
                c_type: "GInitiallyUnowned".into(),
                glib_get_type: "g_initially_unowned_get_type".into(),
                parent: Some(object_tid),
                .. Class::default()
            }));
        lib.add_type(gtk_ns_id, "Widget".into(), Type::Class(
            Class {
                name: "Widget".into(),
                c_type: "GtkWidget".into(),
                glib_get_type: "gtk_widget_get_type".into(),
                parent: Some(ioobject_tid),
                .. Class::default()
            }));
        lib
    }

    #[test]
    fn fill_class_parents() {
        let mut lib = make_library();
        lib.postprocessing();
        let object_tid = lib.find_type(0, "GLib.Object").unwrap();
        let ioobject_tid = lib.find_type(0, "GLib.InitiallyUnowned").unwrap();
        let widget_tid = lib.find_type(0, "Gtk.Widget").unwrap();
        assert_eq!(lib.type_(object_tid).to_ref_as::<Class>().parents, &[]);
        assert_eq!(lib.type_(ioobject_tid).to_ref_as::<Class>().parents, &[object_tid]);
        assert_eq!(lib.type_(widget_tid).to_ref_as::<Class>().parents, &[ioobject_tid, object_tid]);
    }
}
