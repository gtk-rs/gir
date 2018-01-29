use std::collections::HashMap;

use library::*;
use nameutil;
use parser::is_empty_c_type;
use traits::MaybeRefAs;

impl Namespace {
    fn unresolved(&self) -> Vec<&str> {
        self.index
            .iter()
            .filter_map(|(name, &id)| if self.types[id as usize].is_none() {
                Some(&name[..])
            } else {
                None
            })
            .collect()
    }
}

type DetectedCTypes = HashMap<TypeId, String>;

impl Library {
    pub fn postprocessing(&mut self) {
        self.fix_gtype();
        self.check_resolved();
        self.fill_empty_signals_c_types();
        self.resolve_class_structs();
        self.correlate_class_structs();
        self.fix_fields();
    }

    fn fix_gtype(&mut self) {
        if let Some(ns_id) = self.find_namespace("GObject") {
            // hide the `GType` type alias in `GObject`
            self.add_type(ns_id, "Type", Type::Fundamental(Fundamental::Unsupported));
        }
    }

    fn check_resolved(&self) {
        let list: Vec<_> = self.index
            .iter()
            .flat_map(|(name, &id)| {
                let name = name.clone();
                self.namespace(id)
                    .unresolved()
                    .into_iter()
                    .map(move |s| format!("{}.{}", name, s))
            })
            .collect();

        if !list.is_empty() {
            panic!("Incomplete library, unresolved: {:?}", list);
        }
    }

    fn fill_empty_signals_c_types(&mut self) {
        let mut tids = Vec::new();
        let mut c_types = DetectedCTypes::new();
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for (id, type_) in ns.types.iter().enumerate() {
                let type_ = type_.as_ref().unwrap(); //Always contains something
                let tid = TypeId {
                    ns_id: ns_id as u16,
                    id: id as u32,
                };
                match *type_ {
                    Type::Class(ref klass) => {
                        if self.detect_empty_signals_c_types(&klass.signals, &mut c_types) {
                            tids.push(tid);
                        }
                    }
                    Type::Interface(ref iface) => {
                        if self.detect_empty_signals_c_types(&iface.signals, &mut c_types) {
                            tids.push(tid);
                        }
                    }
                    _ => (),
                }
            }
        }

        fn update_empty_signals_c_types(signals: &mut [Signal], c_types: &DetectedCTypes) {
            for signal in signals {
                update_empty_signal_c_types(signal, c_types);
            }
        }

        fn update_empty_signal_c_types(signal: &mut Signal, c_types: &DetectedCTypes) {
            for par in &mut signal.parameters {
                update_empty_c_type(&mut par.c_type, par.typ, c_types);
            }
            update_empty_c_type(&mut signal.ret.c_type, signal.ret.typ, c_types);
        }

        fn update_empty_c_type(c_type: &mut String, tid: TypeId, c_types: &DetectedCTypes) {
            if !is_empty_c_type(c_type) {
                return;
            }
            if let Some(ref mut s) = c_types.get(&tid) {
                *c_type = s.clone();
            }
        }

        for tid in tids {
            match *self.type_mut(tid) {
                Type::Class(ref mut klass) => {
                    update_empty_signals_c_types(&mut klass.signals, &c_types)
                }
                Type::Interface(ref mut iface) => {
                    update_empty_signals_c_types(&mut iface.signals, &c_types)
                }
                _ => (),
            }
        }
    }

    fn detect_empty_signals_c_types(
        &self,
        signals: &[Signal],
        c_types: &mut DetectedCTypes,
    ) -> bool {
        let mut detected = false;
        for signal in signals {
            if self.detect_empty_signal_c_types(signal, c_types) {
                detected = true;
            }
        }
        detected
    }

    fn detect_empty_signal_c_types(&self, signal: &Signal, c_types: &mut DetectedCTypes) -> bool {
        let mut detected = false;
        for par in &signal.parameters {
            if self.detect_empty_c_type(&par.c_type, par.typ, c_types) {
                detected = true;
            }
        }
        if self.detect_empty_c_type(&signal.ret.c_type, signal.ret.typ, c_types) {
            detected = true;
        }
        detected
    }

    fn detect_empty_c_type(&self, c_type: &str, tid: TypeId, c_types: &mut DetectedCTypes) -> bool {
        if !is_empty_c_type(c_type) {
            return false;
        }
        if !c_types.contains_key(&tid) {
            if let Some(detected_c_type) = self.c_type_by_type_id(tid) {
                c_types.insert(tid, detected_c_type);
            }
        }
        true
    }

    fn c_type_by_type_id(&self, tid: TypeId) -> Option<String> {
        let type_ = self.type_(tid);
        type_
            .get_glib_name()
            .map(|glib_name| if self.is_referenced_type(type_) {
                format!("{}*", glib_name)
            } else {
                glib_name.to_string()
            })
    }

    fn is_referenced_type(&self, type_: &Type) -> bool {
        use library::Type::*;
        match *type_ {
            Alias(ref alias) => self.is_referenced_type(self.type_(alias.typ)),
            Record(..) | Union(..) | Class(..) | Interface(..) => true,
            _ => false,
        }
    }

    fn resolve_class_structs(&mut self) {
        // stores pairs of (gtype-struct-c-name, type-name)
        let mut structs_and_types = Vec::new();

        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for type_ in &ns.types {
                let type_ = type_.as_ref().unwrap(); //Always contains something

                if let Type::Record(ref record) = *type_ {
                    if let Some(ref struct_for) = record.gtype_struct_for {
                        if let Some(struct_for_tid) = self.find_type(ns_id as u16, struct_for) {
                            structs_and_types.push ((record.c_type.clone(), struct_for_tid));
                        }
                    }
                }
            }
        }

        for (gtype_struct_c_type, struct_for_tid) in structs_and_types {
            match *self.type_mut(struct_for_tid) {
                Type::Class(ref mut klass) => {
                    klass.c_class_type = Some(gtype_struct_c_type);
                },

                Type::Interface(ref mut iface) => {
                    iface.c_class_type = Some(gtype_struct_c_type);
                },

                ref x => unreachable!("Something other than a class or interface has a class struct: {:?}", x)
            }
        }
    }

    fn correlate_class_structs(&self) {
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for type_ in &ns.types {
                let type_ = type_.as_ref().unwrap(); //Always contains something
                let name;
                let type_struct;
                let c_class_type;

                match *type_ {
                    Type::Class(ref klass) => {
                        name         = &klass.name;
                        type_struct  = &klass.type_struct;
                        c_class_type = &klass.c_class_type;
                    },

                    Type::Interface(ref iface) => {
                        name         = &iface.name;
                        type_struct  = &iface.type_struct;
                        c_class_type = &iface.c_class_type;
                    },

                    _ => { continue; }
                }

                if let Some(ref type_struct) = *type_struct {
                    let type_struct_tid = self.find_type(ns_id as u16, type_struct);
                    if type_struct_tid.is_none() {
                        panic!("\"{}\" has glib:type-struct=\"{}\" but there is no such record",
                               name,
                               type_struct);
                    }

                    let type_struct_type = self.type_(type_struct_tid.unwrap());

                    if let Type::Record(ref r) = *type_struct_type {
                        if r.gtype_struct_for.as_ref() != Some(name) {
                            if let Some(ref gtype_struct_for) = r.gtype_struct_for {
                                panic!("\"{}\" has glib:type-struct=\"{}\" but the corresponding record \"{}\" has glib:is-gtype-struct-for={:?}",
                                       name,
                                       type_struct,
                                       r.name,
                                       gtype_struct_for);
                            } else {
                                panic!("\"{}\" has glib:type-struct=\"{}\" but the corresponding record \"{}\" has no glib:is-gtype-struct-for attribute",
                                       name,
                                       type_struct,
                                       r.name);
                            }
                        }
                    } else {
                        panic!("Element with name=\"{}\" should be a record but it isn't",
                               type_struct);
                    }
                } else if let Some(ref c) = *c_class_type {
                    panic!("\"{}\" has no glib:type-struct but there is an element with glib:is-gtype-struct-for=\"{}\"",
                           name,
                           c);
                }
                // else both type_struct and c_class_type are None,
                // and that's fine because they don't reference each
                // other.
            }
        }
    }

    fn fix_fields(&mut self) {
        enum Action {
            SetCType(String),
            SetName(String),
        }
        let mut actions: Vec<(TypeId, usize, Action)> = Vec::new();
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for (id, type_) in ns.types.iter().enumerate() {
                let type_ = type_.as_ref().unwrap(); //Always contains something
                let tid = TypeId {
                    ns_id: ns_id as u16,
                    id: id as u32,
                };
                match *type_ {
                    Type::Class(Class { ref name, ref fields, ..}) |
                    Type::Record(Record { ref name, ref fields, ..}) |
                    Type::Union(Union { ref name, ref fields, ..}) => {
                        for (fid, field) in fields.iter().enumerate() {
                            if nameutil::needs_mangling(&field.name) {
                                let new_name = nameutil::mangle_keywords(&*field.name).to_string();
                                actions.push((tid, fid, Action::SetName(new_name)));
                            }
                            if field.c_type.is_some() {
                                continue;
                            }
                            let field_type = self.type_(field.typ);
                            if let Some(_) = field_type.maybe_ref_as::<Function>() {
                                // Function pointers generally don't have c_type.
                                continue;
                            }
                            if let Some(c_type) = field_type.get_glib_name() {
                                actions.push((tid, fid, Action::SetCType(c_type.to_owned())));
                                continue;
                            }
                            if let Type::Fundamental(Fundamental::Pointer) = *field_type {
                                // For example SoupBuffer is missing c:type for data field.
                                actions.push((tid, fid, Action::SetCType("void*".to_owned())));
                                continue;
                            }
                            error!("Field `{}::{}` is missing c:type", name, &field.name);
                        }
                    },
                    _ => {},
                }
            }
        }
        for (tid, fid, action) in actions {
            match *self.type_mut(tid) {
                Type::Class(Class { ref name, ref mut fields, ..}) |
                Type::Record(Record { ref name, ref mut fields, ..}) |
                Type::Union(Union { ref name, ref mut fields, ..}) => {
                    match action {
                        Action::SetCType(c_type) => {
                            warn!("Field `{}::{}` missing c:type assumed to be `{}`",
                                  name, &fields[fid].name, c_type);
                            fields[fid].c_type = Some(c_type);
                        }
                        Action::SetName(name) => fields[fid].name = name,
                    }
                },
                _ => unreachable!("Expected class, record or union"),
            }
        }
    }
}
