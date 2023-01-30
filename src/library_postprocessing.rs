use std::collections::HashMap;

use log::{error, info};

use crate::{
    analysis::types::IsIncomplete,
    config::{
        gobjects::{GObject, GStatus},
        matchable::Matchable,
        Config, WorkMode,
    },
    library::*,
    nameutil,
    parser::is_empty_c_type,
    traits::MaybeRefAs,
};

impl Namespace {
    fn unresolved(&self) -> Vec<&str> {
        self.index
            .iter()
            .filter_map(|(name, &id)| {
                if self.types[id as usize].is_none() {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect()
    }
}

type DetectedCTypes = HashMap<TypeId, String>;

impl Library {
    pub fn postprocessing(&mut self, config: &Config) {
        self.fix_gtype();
        self.check_resolved();
        self.fill_empty_signals_c_types();
        self.resolve_class_structs();
        self.correlate_class_structs();
        self.fix_fields();
        self.make_unrepresentable_types_opaque();
        self.mark_final_types(config);
        self.update_error_domain_functions(config);
        self.mark_ignored_enum_members(config);
    }

    fn fix_gtype(&mut self) {
        if let Some(ns_id) = self.find_namespace("GObject") {
            // hide the `GType` type alias in `GObject`
            self.add_type(ns_id, "Type", Type::Basic(Basic::Unsupported));
        }
    }

    fn check_resolved(&self) {
        let list: Vec<_> = self
            .index
            .iter()
            .flat_map(|(name, &id)| {
                let name = name.clone();
                self.namespace(id)
                    .unresolved()
                    .into_iter()
                    .map(move |s| format!("{name}.{s}"))
            })
            .collect();

        assert!(list.is_empty(), "Incomplete library, unresolved: {list:?}");
    }

    fn fill_empty_signals_c_types(&mut self) {
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
            if let Some(s) = c_types.get(&tid) {
                *c_type = s.clone();
            }
        }

        let mut tids = Vec::new();
        let mut c_types = DetectedCTypes::new();
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for (id, type_) in ns.types.iter().enumerate() {
                let type_ = type_.as_ref().unwrap(); // Always contains something
                let tid = TypeId {
                    ns_id: ns_id as u16,
                    id: id as u32,
                };
                match type_ {
                    Type::Class(klass) => {
                        if self.detect_empty_signals_c_types(&klass.signals, &mut c_types) {
                            tids.push(tid);
                        }
                    }
                    Type::Interface(iface) => {
                        if self.detect_empty_signals_c_types(&iface.signals, &mut c_types) {
                            tids.push(tid);
                        }
                    }
                    _ => (),
                }
            }
        }

        for tid in tids {
            match self.type_mut(tid) {
                Type::Class(klass) => update_empty_signals_c_types(&mut klass.signals, &c_types),
                Type::Interface(iface) => {
                    update_empty_signals_c_types(&mut iface.signals, &c_types);
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
        if let std::collections::hash_map::Entry::Vacant(entry) = c_types.entry(tid) {
            if let Some(detected_c_type) = self.c_type_by_type_id(tid) {
                entry.insert(detected_c_type);
            }
        }
        true
    }

    fn c_type_by_type_id(&self, tid: TypeId) -> Option<String> {
        let type_ = self.type_(tid);
        type_.get_glib_name().map(|glib_name| {
            if self.is_referenced_type(type_) {
                format!("{glib_name}*")
            } else {
                glib_name.to_string()
            }
        })
    }

    fn is_referenced_type(&self, type_: &Type) -> bool {
        use crate::library::Type::*;
        match type_ {
            Alias(alias) => self.is_referenced_type(self.type_(alias.typ)),
            Record(..) | Union(..) | Class(..) | Interface(..) => true,
            _ => false,
        }
    }

    fn resolve_class_structs(&mut self) {
        // stores pairs of (gtype-struct-c-name, type-name)
        let mut structs_and_types = Vec::new();

        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for type_ in &ns.types {
                let type_ = type_.as_ref().unwrap(); // Always contains something

                if let Type::Record(record) = type_ {
                    if let Some(ref struct_for) = record.gtype_struct_for {
                        if let Some(struct_for_tid) = self.find_type(ns_id as u16, struct_for) {
                            structs_and_types.push((record.c_type.clone(), struct_for_tid));
                        }
                    }
                }
            }
        }

        for (gtype_struct_c_type, struct_for_tid) in structs_and_types {
            match self.type_mut(struct_for_tid) {
                Type::Class(klass) => {
                    klass.c_class_type = Some(gtype_struct_c_type);
                }

                Type::Interface(iface) => {
                    iface.c_class_type = Some(gtype_struct_c_type);
                }

                x => unreachable!(
                    "Something other than a class or interface has a class struct: {:?}",
                    x
                ),
            }
        }
    }

    fn correlate_class_structs(&self) {
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for type_ in &ns.types {
                let type_ = type_.as_ref().unwrap(); // Always contains something
                let name;
                let type_struct;
                let c_class_type;

                match type_ {
                    Type::Class(klass) => {
                        name = &klass.name;
                        type_struct = &klass.type_struct;
                        c_class_type = &klass.c_class_type;
                    }

                    Type::Interface(iface) => {
                        name = &iface.name;
                        type_struct = &iface.type_struct;
                        c_class_type = &iface.c_class_type;
                    }

                    _ => {
                        continue;
                    }
                }

                if let Some(type_struct) = type_struct {
                    let type_struct_tid = self.find_type(ns_id as u16, type_struct);
                    assert!(
                        type_struct_tid.is_some(),
                        "\"{name}\" has glib:type-struct=\"{type_struct}\" but there is no such record"
                    );

                    let type_struct_type = self.type_(type_struct_tid.unwrap());

                    if let Type::Record(r) = type_struct_type {
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
                        panic!(
                            "Element with name=\"{type_struct}\" should be a record but it isn't"
                        );
                    }
                } else if let Some(c) = c_class_type {
                    panic!("\"{name}\" has no glib:type-struct but there is an element with glib:is-gtype-struct-for=\"{c}\"");
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
                let type_ = type_.as_ref().unwrap(); // Always contains something
                let tid = TypeId {
                    ns_id: ns_id as u16,
                    id: id as u32,
                };
                match type_ {
                    Type::Class(Class { name, fields, .. })
                    | Type::Record(Record { name, fields, .. })
                    | Type::Union(Union { name, fields, .. }) => {
                        for (fid, field) in fields.iter().enumerate() {
                            if nameutil::needs_mangling(&field.name) {
                                let new_name = nameutil::mangle_keywords(&*field.name).into_owned();
                                actions.push((tid, fid, Action::SetName(new_name)));
                            }
                            if field.c_type.is_some() {
                                continue;
                            }
                            let field_type = self.type_(field.typ);
                            if field_type.maybe_ref_as::<Function>().is_some() {
                                // Function pointers generally don't have c_type.
                                continue;
                            }
                            if let Some(c_type) = field_type.get_glib_name() {
                                actions.push((tid, fid, Action::SetCType(c_type.to_owned())));
                                continue;
                            }
                            if let Type::Basic(Basic::Pointer) = field_type {
                                // For example SoupBuffer is missing c:type for data field.
                                actions.push((tid, fid, Action::SetCType("void*".to_owned())));
                                continue;
                            }
                            if let Type::FixedArray(..) = field_type {
                                // fixed-size Arrays can only have inner c_type
                                // HACK: field c_type used only in sys mode for pointer checking
                                // so any string without * will work
                                let array_c_type = "fixed_array".to_owned();
                                actions.push((tid, fid, Action::SetCType(array_c_type)));
                                continue;
                            }
                            error!("Field `{}::{}` is missing c:type", name, &field.name);
                        }
                    }
                    _ => {}
                }
            }
        }
        let ignore_missing_ctype = ["padding", "reserved", "_padding", "_reserved"];
        for (tid, fid, action) in actions {
            match self.type_mut(tid) {
                Type::Class(Class { name, fields, .. })
                | Type::Record(Record { name, fields, .. })
                | Type::Union(Union { name, fields, .. }) => match action {
                    Action::SetCType(c_type) => {
                        // Don't be verbose when internal fields such as padding don't provide a
                        // c-type
                        if !ignore_missing_ctype.contains(&fields[fid].name.as_str()) {
                            warn_main!(
                                tid,
                                "Field `{}::{}` missing c:type assumed to be `{}`",
                                name,
                                &fields[fid].name,
                                c_type
                            );
                        }
                        fields[fid].c_type = Some(c_type);
                    }
                    Action::SetName(name) => fields[fid].name = name,
                },
                _ => unreachable!("Expected class, record or union"),
            }
        }
    }

    fn make_unrepresentable_types_opaque(&mut self) {
        // Unions with non-`Copy` fields are unstable (see issue #32836).
        // It would seem that this shouldn't be cause for concern as one can
        // always make all types in the union copyable.
        //
        // Unfortunately, this is not that simple, as some types are currently
        // unrepresentable in Rust, and they do occur inside the unions.
        // Thus to avoid the problem, we mark all unions with such unrepresentable
        // types as opaque, and don't generate their definitions.
        let mut unrepresentable: Vec<TypeId> = Vec::new();
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for (id, type_) in ns.types.iter().enumerate() {
                let type_ = type_.as_ref().unwrap();
                let tid = TypeId {
                    ns_id: ns_id as u16,
                    id: id as u32,
                };
                match type_ {
                    Type::Union(Union { fields, .. }) if fields.as_slice().is_incomplete(self) => {
                        unrepresentable.push(tid);
                    }
                    _ => {}
                }
            }
        }
        for tid in unrepresentable {
            match self.type_mut(tid) {
                Type::Union(Union { name, fields, .. }) => {
                    info!("Type `{}` is not representable.", name);
                    fields.clear();
                }
                _ => unreachable!("Expected a union"),
            }
        }
    }

    fn has_subtypes(&self, parent_tid: TypeId) -> bool {
        for (tid, _) in self.types() {
            if let Type::Class(class) = self.type_(tid) {
                if class.parent == Some(parent_tid) {
                    return true;
                }
            }
        }

        false
    }

    fn mark_final_types(&mut self, config: &Config) {
        // Here we mark all class types as final types if configured so in the config or
        // otherwise if there is no public class struct for the type or the instance
        // struct has no fields (i.e. is not known!), and there are no known
        // subtypes.
        //
        // Final types can't have any subclasses and we handle them slightly different
        // for that reason.
        // FIXME: without class_hierarchy this function O(n2) due inner loop in
        // `has_subtypes`
        let mut final_types: Vec<TypeId> = Vec::new();

        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for (id, type_) in ns.types.iter().enumerate() {
                let type_ = type_.as_ref().unwrap(); // Always contains something

                if let Type::Class(klass) = type_ {
                    let tid = TypeId {
                        ns_id: ns_id as u16,
                        id: id as u32,
                    };

                    let full_name = tid.full_name(self);
                    let obj = config.objects.get(&*full_name);

                    let is_final = if let Some(GObject {
                        final_type: Some(final_type),
                        ..
                    }) = obj
                    {
                        // The config might also be used to override a type that is wrongly
                        // detected as final type otherwise
                        *final_type
                    } else if klass.type_struct.is_none() {
                        !self.has_subtypes(tid)
                    } else {
                        let has_subtypes = self.has_subtypes(tid);
                        let instance_struct_known = !klass.fields.is_empty();

                        let class_struct_known = if let Some(class_record_tid) =
                            self.find_type(ns_id as u16, klass.type_struct.as_ref().unwrap())
                        {
                            if let Type::Record(record) = self.type_(class_record_tid) {
                                !record.disguised
                            } else {
                                unreachable!("Type {} with non-record class", full_name);
                            }
                        } else {
                            unreachable!("Can't find class for {}", full_name);
                        };

                        !has_subtypes && (!instance_struct_known || !class_struct_known)
                    };
                    if is_final {
                        final_types.push(tid);
                    }
                }
            }
        }

        for tid in final_types {
            if let Type::Class(Class { final_type, .. }) = self.type_mut(tid) {
                *final_type = true;
            } else {
                unreachable!();
            }
        }
    }

    fn update_error_domain_functions(&mut self, config: &Config) {
        // Find find all error domains that have corresponding functions
        let mut error_domains = vec![];
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            'next_enum: for (id, type_) in ns.types.iter().enumerate() {
                let type_ = type_.as_ref().unwrap(); // Always contains something
                let enum_tid = TypeId {
                    ns_id: ns_id as u16,
                    id: id as u32,
                };

                if let Type::Enumeration(enum_) = type_ {
                    if let Some(ErrorDomain::Quark(ref domain)) = enum_.error_domain {
                        let domain = domain.replace('-', "_");

                        let mut function_candidates = vec![domain.clone()];
                        if !domain.ends_with("_quark") {
                            function_candidates.push(format!("{domain}_quark"));
                        }
                        if !domain.ends_with("_error_quark") {
                            if domain.ends_with("_quark") {
                                function_candidates
                                    .push(format!("{}_error_quark", &domain[..(domain.len() - 6)]));
                            } else {
                                function_candidates.push(format!("{domain}_error_quark"));
                            }
                        }
                        if let Some(domain) = domain.strip_suffix("_error_quark") {
                            function_candidates.push(domain.to_owned());
                        }
                        if let Some(domain) = domain.strip_suffix("_quark") {
                            function_candidates.push(domain.to_owned());
                        }

                        if let Some(func) = ns.functions.iter().find(|f| {
                            function_candidates
                                .iter()
                                .any(|c| f.c_identifier.as_ref() == Some(c))
                        }) {
                            error_domains.push((
                                ns_id,
                                enum_tid,
                                None,
                                func.c_identifier.as_ref().unwrap().clone(),
                            ));
                            continue 'next_enum;
                        }

                        // Quadratic in number of types...
                        for (id, type_) in ns.types.iter().enumerate() {
                            let type_ = type_.as_ref().unwrap(); // Always contains something
                            let domain_tid = TypeId {
                                ns_id: ns_id as u16,
                                id: id as u32,
                            };

                            let functions = match type_ {
                                Type::Enumeration(Enumeration { functions, .. })
                                | Type::Class(Class { functions, .. })
                                | Type::Record(Record { functions, .. })
                                | Type::Interface(Interface { functions, .. }) => functions,
                                _ => continue,
                            };

                            if let Some(func) = functions.iter().find(|f| {
                                function_candidates
                                    .iter()
                                    .any(|c| f.c_identifier.as_ref() == Some(c))
                            }) {
                                error_domains.push((
                                    ns_id,
                                    enum_tid,
                                    Some(domain_tid),
                                    func.c_identifier.as_ref().unwrap().clone(),
                                ));
                                continue 'next_enum;
                            }
                        }
                    }
                }
            }
        }

        for (ns_id, enum_tid, domain_tid, function_name) in error_domains {
            if config.work_mode != WorkMode::Sys {
                if let Some(domain_tid) = domain_tid {
                    match self.type_mut(domain_tid) {
                        Type::Enumeration(Enumeration { functions, .. })
                        | Type::Class(Class { functions, .. })
                        | Type::Record(Record { functions, .. })
                        | Type::Interface(Interface { functions, .. }) => {
                            let pos = functions
                                .iter()
                                .position(|f| f.c_identifier.as_ref() == Some(&function_name))
                                .unwrap();
                            functions.remove(pos);
                        }
                        _ => unreachable!(),
                    }
                } else {
                    let pos = self.namespaces[ns_id]
                        .functions
                        .iter()
                        .position(|f| f.c_identifier.as_ref() == Some(&function_name))
                        .unwrap();
                    self.namespaces[ns_id].functions.remove(pos);
                }
            }

            if let Type::Enumeration(enum_) = self.type_mut(enum_tid) {
                assert!(enum_.error_domain.is_some());
                enum_.error_domain = Some(ErrorDomain::Function(function_name));
            } else {
                unreachable!();
            }
        }
    }

    fn mark_ignored_enum_members(&mut self, config: &Config) {
        let mut members_to_change = vec![];
        for (ns_id, ns) in self.namespaces.iter().enumerate() {
            for (id, _type_) in ns.types.iter().enumerate() {
                let type_id = TypeId {
                    ns_id: ns_id as u16,
                    id: id as u32,
                };

                match self.type_(type_id) {
                    Type::Bitfield(Bitfield { name, members, .. })
                    | Type::Enumeration(Enumeration { name, members, .. }) => {
                        let full_name = format!("{}.{}", ns.name, name);
                        let config = config.objects.get(&full_name);
                        let mut type_members = HashMap::new();
                        for member in members.iter() {
                            let status = config.and_then(|m| {
                                m.members.matched(&member.name).first().map(|m| m.status)
                            });
                            type_members.insert(member.c_identifier.clone(), status);
                        }
                        members_to_change.push((type_id, type_members));
                    }
                    _ => (),
                };
            }
        }

        for (type_id, item_members) in members_to_change {
            match self.type_mut(type_id) {
                Type::Bitfield(Bitfield { members, .. })
                | Type::Enumeration(Enumeration { members, .. }) => {
                    for member in members.iter_mut() {
                        let status = item_members
                            .get(&member.c_identifier)
                            .copied()
                            .flatten()
                            .unwrap_or(GStatus::Generate);
                        member.status = status;
                    }
                }
                _ => (),
            };
        }
    }
}
