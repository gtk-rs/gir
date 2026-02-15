use std::path::{Path, PathBuf};

use log::{trace, warn};

use crate::{library::*, version::Version};

use gir_parser::{Namespace, Repository, prelude::*};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DocFormat {
    GtkDocMarkdown,
    GtkDocDocbook,
    GiDocgen,
    Hotdoc,
    #[default]
    Unknown,
}

impl std::str::FromStr for DocFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gtk-doc-markdown" => Ok(Self::GtkDocMarkdown),
            "gtk-doc-docbook" => Ok(Self::GtkDocDocbook),
            "gi-docgen" => Ok(Self::GiDocgen),
            "hotdoc" => Ok(Self::Hotdoc),
            "unknown" => Ok(Self::Unknown),
            e => Err(format!("Invalid doc:format {e}")),
        }
    }
}

const EMPTY_CTYPE: &str = "/*EMPTY*/";

pub fn is_empty_c_type(c_type: &str) -> bool {
    c_type == EMPTY_CTYPE
}

impl Library {
    pub fn read_file<P: AsRef<Path>>(
        &mut self,
        dirs: &[P],
        libs: &mut Vec<String>,
    ) -> Result<(), String> {
        trace!(
            "Reading files {:#?} in dirs={:#?}",
            libs,
            dirs.iter()
                .map(|p| p.as_ref().display())
                .collect::<Vec<_>>()
        );
        for dir in dirs {
            let dir: &Path = dir.as_ref();
            let file_name = make_file_name(dir, &libs[libs.len() - 1]);
            warn!("repo {}", file_name.display());
            let Ok(mut repo) = Repository::from_path(&file_name) else {
                warn!("couldn't parse repository");
                continue;
            };
            self.read_repository(dirs, &mut repo, libs)?;
            return Ok(());
        }
        Err(format!("Couldn't find `{}`...", &libs[libs.len() - 1]))
    }

    fn read_repository<P: AsRef<Path>>(
        &mut self,
        dirs: &[P],
        repo: &mut Repository,
        libs: &mut Vec<String>,
    ) -> Result<(), String> {
        trace!(
            "Reading repository identifier={:#?},symbol={:#?}",
            repo.c_identifier_prefixes().collect::<Vec<_>>().join(", "),
            repo.c_symbol_prefixes().collect::<Vec<_>>().join(", "),
        );
        self.read_namespace(repo, repo.namespace())?;
        for include in repo.namespace_includes() {
            let (name, version) = (include.name(), include.version());
            trace!("Checking namespace={name},version={version}");
            trace!("wtf={:#?}", self.index);
            let namespace = self.find_namespace(name);
            // If namespace is not found or not parsed yet
            if namespace.is_none() || namespace.is_some_and(|n| !n.1) {
                let lib = format!("{name}-{version}");
                warn!("dealing with {lib}");
                if libs.contains(&lib) {
                    return Err(format!(
                        "`{}` includes itself (full path:`{}`)!",
                        lib,
                        libs.join("::")
                    ));
                }
                warn!(
                    "trying out {lib} in {:#?}",
                    dirs.iter().map(|d| d.as_ref()).collect::<Vec<_>>()
                );
                libs.push(lib);
                self.read_file(dirs, libs)?;
                libs.pop();
            } else {
                trace!("Namespace={name},version={version} found");
            }
        }
        Ok(())
    }

    fn read_namespace(&mut self, repo: &Repository, namespace: &Namespace) -> Result<(), String> {
        let ns_name = namespace.name();
        let ns_id = self.add_namespace(ns_name, true);
        trace!("Namespace {ns_name} added with ns_id={ns_id} assigned");

        {
            let ns = self.namespace_mut(ns_id);
            ns.package_names = repo
                .packages()
                .iter()
                .map(|p| p.name().to_owned())
                .collect::<Vec<_>>();
            ns.c_includes = repo
                .header_includes()
                .iter()
                .map(|p| p.name().to_owned())
                .collect::<Vec<_>>();
            if let Some(s) = namespace.shared_library() {
                ns.shared_library = s
                    .split(',')
                    .filter_map(|x| {
                        if !x.is_empty() {
                            Some(String::from(x))
                        } else {
                            None
                        }
                    })
                    .collect();
            }
            ns.identifier_prefixes = namespace
                .c_identifier_prefixes()
                .map(String::from)
                .collect();
            ns.symbol_prefixes = namespace.c_symbol_prefixes().map(String::from).collect();
        }

        trace!("Reading {}-{}", ns_name, namespace.version());

        trace!("Reading classes");
        for class in namespace.classes() {
            self.read_class(ns_id, class)?;
        }

        trace!("Reading records");
        for record in namespace.records() {
            self.read_record(ns_id, record, None, None)?;
        }

        trace!("Reading global functions");
        for function in namespace.functions() {
            if function.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, function, FunctionKind::Global)?;
            self.add_function(ns_id, f);
        }

        trace!("Reading callbacks");
        for callback in namespace.callbacks() {
            let f = self.read_callback(ns_id, callback)?;
            self.add_type(ns_id, &f.name.clone(), Type::Function(f));
        }

        trace!("Reading aliases");
        for alias in namespace.aliases() {
            self.read_alias(ns_id, alias)?;
        }

        trace!("Reading constants");
        for constant in namespace.constants() {
            self.read_constant(ns_id, constant)?;
        }

        trace!("Reading enumerations");
        for enumeration in namespace.enums() {
            self.read_enumeration(ns_id, enumeration)?;
        }

        trace!("Reading records");
        for record in namespace.records() {
            if let Some(typ) = self.read_record(ns_id, record, None, None)? {
                let name = typ.get_name();
                self.add_type(ns_id, &name, typ);
            }
        }

        trace!("Reading unions");
        for union in namespace.unions() {
            trace!(
                "Reading union name={:#?},c:type={:#?}",
                union.name(),
                union.c_type()
            );
            self.read_named_union(ns_id, union)?;
        }

        trace!("Reading interfaces");
        for interface in namespace.interfaces() {
            trace!(
                "Reading interface name={},c:type={:#?}",
                interface.name(),
                interface.c_type()
            );
            self.read_interface(ns_id, interface)?;
        }

        trace!("Reading bitfields");
        for bitfield in namespace.flags() {
            // should be renamed upstream maybe?
            trace!(
                "Reading bitfield name={},c:type={}",
                bitfield.name(),
                bitfield.c_type()
            );
            self.read_bitfield(ns_id, bitfield)?;
        }
        Ok(())
    }

    fn read_class(&mut self, ns_id: u16, elem: &gir_parser::Class) -> Result<(), String> {
        let name = elem.name().to_owned();
        let c_type = elem.c_type().unwrap_or(elem.g_type_name()).to_owned();
        let symbol_prefix = elem
            .symbol_prefix()
            .map(ToOwned::to_owned)
            .ok_or_else(|| format!("<class> {} doesn't have a `symbol-prefix` attribute", name))?;
        let type_struct = elem.g_type_struct().map(ToOwned::to_owned);
        let glib_get_type = elem.g_get_type().to_owned();
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());
        let is_fundamental = elem.is_fundamental();
        let (ref_fn, unref_fn) = if is_fundamental {
            (
                elem.g_ref_func().map(ToOwned::to_owned),
                elem.g_unref_func().map(ToOwned::to_owned),
            )
        } else {
            (None, None)
        };

        let is_abstract = elem.is_abstract();
        let final_type = elem.is_final();

        let mut fns = Vec::new();
        let mut signals = Vec::new();
        let mut properties = Vec::new();
        let mut impls = Vec::new();
        let mut fields = Vec::new();
        let mut vfns = Vec::new();
        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);
        let mut union_count = 1;

        for constructor in elem.constructors() {
            if constructor.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, constructor, FunctionKind::Constructor)?;
            fns.push(f);
        }
        for function in elem.functions() {
            if function.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, function, FunctionKind::Function)?;
            fns.push(f);
        }
        for method in elem.methods() {
            if method.moved_to().is_some() {
                continue;
            }
            let f = self.read_method(ns_id, method)?;
            fns.push(f);
        }
        for signal in elem.signals() {
            let signal = self.read_signal(ns_id, signal)?;
            signals.push(signal);
        }
        for property in elem.properties() {
            properties.push(self.read_property(ns_id, property, &symbol_prefix)?);
        }

        for implement in elem.implements() {
            impls.push(self.find_or_stub_type(ns_id, implement.name()));
        }

        for field in elem.fields() {
            if let gir_parser::ClassField::Field(field) = field {
                fields.push(self.read_field(ns_id, field)?);
            }
        }

        for virtual_method in elem.virtual_methods() {
            if virtual_method.moved_to().is_some() {
                continue;
            }
            let f = self.read_virtual_method(ns_id, virtual_method)?;
            vfns.push(f);
        }

        for field in elem.fields() {
            let gir_parser::ClassField::Union(union) = field else {
                continue;
            };
            let mut u = self.read_union(ns_id, union, Some(&name), Some(&c_type))?;
            let field_name = if let Some(field_name) = union.name() {
                field_name.into()
            } else {
                format!("u{union_count}")
            };

            u = Union {
                name: format!("{name}_{field_name}"),
                c_type: Some(format!("{c_type}_{field_name}")),
                ..u
            };

            let u_doc = u.doc.clone();
            let ctype = u.c_type.clone();

            fields.push(Field {
                name: field_name,
                typ: Type::union(self, u, ns_id),
                doc: u_doc,
                c_type: ctype,
                ..Field::default()
            });
            union_count += 1;
        }

        let parent = elem.parent().map(|s| self.find_or_stub_type(ns_id, s));
        let typ = Type::Class(Class {
            name: name.clone(),
            c_type,
            type_struct,
            c_class_type: None, // this will be resolved during postprocessing
            glib_get_type,
            fields,
            functions: fns,
            virtual_methods: vfns,
            signals,
            properties,
            parent,
            implements: impls,
            final_type,
            doc,
            doc_deprecated,
            version,
            deprecated_version,
            symbol_prefix,
            is_abstract,
            is_fundamental,
            ref_fn,
            unref_fn,
        });
        self.add_type(ns_id, &name, typ);
        Ok(())
    }

    fn read_record(
        &mut self,
        ns_id: u16,
        elem: &gir_parser::Record,
        parent_name_prefix: Option<&str>,
        parent_ctype_prefix: Option<&str>,
    ) -> Result<Option<Type>, String> {
        let record_name = elem.name().unwrap_or_default();
        // Records starting with `_` are intended to be private and should not be bound
        if record_name.starts_with('_') {
            return Ok(None);
        }
        let is_class_record = record_name.ends_with("Class");

        let c_type = elem.c_type().unwrap_or_default();
        if c_type.is_empty() {
            warn!("Found empty record {record_name}");
        }
        let symbol_prefix = elem.symbol_prefix().map(ToOwned::to_owned);
        let get_type = elem.g_get_type().map(ToOwned::to_owned);
        let gtype_struct_for = elem.g_is_gtype_struct_for();
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());
        let pointer = elem.is_pointer();
        let disguised = elem.is_disguised();

        let mut fields = Vec::new();
        let mut fns = Vec::new();
        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);
        let mut union_count = 1;
        for field in elem.fields() {
            let gir_parser::RecordField::Union(union) = field else {
                continue;
            };
            let mut u = self.read_union(ns_id, union, Some(record_name), Some(c_type))?;
            let field_name = if let Some(field_name) = union.name() {
                field_name.into()
            } else {
                format!("u{union_count}")
            };

            u = Union {
                name: format!(
                    "{}{}_{}",
                    parent_name_prefix.map_or_else(String::new, |s| { format!("{s}_") }),
                    record_name,
                    field_name
                ),
                c_type: Some(format!(
                    "{}{}_{}",
                    parent_ctype_prefix.map_or_else(String::new, |s| { format!("{s}_") }),
                    c_type,
                    field_name
                )),
                ..u
            };

            let u_doc = u.doc.clone();
            let ctype = u.c_type.clone();

            fields.push(Field {
                name: field_name,
                typ: Type::union(self, u, ns_id),
                doc: u_doc,
                c_type: ctype,
                ..Field::default()
            });
            union_count += 1;
        }

        for constructor in elem.constructors().iter() {
            if constructor.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, constructor, FunctionKind::Constructor)?;
            fns.push(f);
        }
        for method in elem.methods().iter() {
            if method.moved_to().is_some() {
                continue;
            }
            let f = self.read_method(ns_id, method)?;
            fns.push(f);
        }
        for function in elem.functions().iter() {
            if function.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, function, FunctionKind::Function)?;
            fns.push(f);
        }

        for field in elem.fields() {
            let gir_parser::RecordField::Field(field) = field else {
                continue;
            };
            let mut f = self.read_field(ns_id, field)?;
            // Workaround for bitfields
            if c_type == "GDate" {
                if f.name == "julian_days" {
                    fields.push(f);
                } else if f.name == "julian" {
                    f.name = "flags_dmy".into();
                    f.typ = TypeId::tid_uint32();
                    f.c_type = Some("guint".into());
                    f.bits = None;
                    fields.push(f);
                } else {
                    // Skip
                }
                continue;
            }
            // Workaround for wrong GValue c:type
            if c_type == "GValue" && f.name == "data" {
                f.c_type = Some("GValue_data".into());
            }
            fields.push(f);
        }

        let typ = Type::Record(Record {
            name: record_name.into(),
            c_type: c_type.into(),
            glib_get_type: get_type,
            functions: if is_class_record && gtype_struct_for.is_some() {
                fns.into_iter()
                    .map(|mut f| {
                        f.kind = FunctionKind::ClassMethod;
                        f
                    })
                    .collect::<Vec<_>>()
            } else {
                fns
            },
            gtype_struct_for: gtype_struct_for.map(|s| s.into()),
            fields,
            version,
            deprecated_version,
            doc,
            doc_deprecated,
            disguised,
            pointer,
            symbol_prefix,
        });

        Ok(Some(typ))
    }

    fn read_named_union(&mut self, ns_id: u16, elem: &gir_parser::Union) -> Result<(), String> {
        // Require a name here
        elem.name()
            .ok_or_else(|| String::from("Name is required"))?;

        let mut union = self.read_union(ns_id, elem, None, None)?;
        assert_ne!(union.name, "");
        // Workaround for missing c:type
        if union.name == "_Value__data__union" {
            union.c_type = Some("GValue_data".into());
        } else if union.c_type.is_none() {
            return Err(String::from("Missing union c:type"));
        }
        self.add_type(ns_id, &union.name.clone(), Type::Union(union));
        Ok(())
    }

    fn read_union(
        &mut self,
        ns_id: u16,
        elem: &gir_parser::Union,
        parent_name_prefix: Option<&str>,
        parent_ctype_prefix: Option<&str>,
    ) -> Result<Union, String> {
        let name = elem.name().unwrap_or("").to_owned();
        let c_type = elem.c_type().unwrap_or("").to_owned();
        if c_type.is_empty() {
            warn!("Found empty union {name}");
        }
        let get_type = elem.g_get_type().map(ToOwned::to_owned);
        let symbol_prefix = elem.c_symbol_prefix().map(ToOwned::to_owned);

        let mut fields = Vec::new();
        let mut fns = Vec::new();
        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);

        let mut struct_count = 1;

        for field in elem.fields() {
            let gir_parser::UnionField::Record(record) = field else {
                continue;
            };
            let Some(Type::Record(mut r)) =
                self.read_record(ns_id, record, parent_name_prefix, parent_ctype_prefix)?
            else {
                continue;
            };

            let field_name = if let Some(field_name) = record.name() {
                field_name.into()
            } else {
                format!("s{struct_count}")
            };

            r = Record {
                name: format!(
                    "{}{}_{}",
                    parent_name_prefix.map_or_else(String::new, |s| { format!("{s}_") }),
                    name,
                    field_name
                ),
                c_type: format!(
                    "{}{}_{}",
                    parent_ctype_prefix.map_or_else(String::new, |s| { format!("{s}_") }),
                    c_type,
                    field_name
                ),
                ..r
            };

            let r_doc = r.doc.clone();
            let ctype = r.c_type.clone();

            fields.push(Field {
                name: field_name,
                typ: Type::record(self, r, ns_id),
                doc: r_doc,
                c_type: Some(ctype),
                ..Field::default()
            });

            struct_count += 1;
        }

        for field in elem.fields() {
            if let gir_parser::UnionField::Field(field) = field {
                fields.push(self.read_field(ns_id, field)?);
            };
        }

        for constructor in elem.constructors().iter() {
            if constructor.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, constructor, FunctionKind::Constructor)?;
            fns.push(f);
        }
        for method in elem.methods().iter() {
            if method.moved_to().is_some() {
                continue;
            }
            let f = self.read_method(ns_id, method)?;
            fns.push(f);
        }
        for function in elem.functions().iter() {
            if function.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, function, FunctionKind::Function)?;
            fns.push(f);
        }
        Ok(Union {
            name,
            c_type: Some(c_type),
            glib_get_type: get_type,
            fields,
            functions: fns,
            doc,
            symbol_prefix,
        })
    }

    fn read_virtual_method(
        &mut self,
        ns_id: u16,
        elem: &gir_parser::VirtualMethod,
    ) -> Result<Function, String> {
        let name = elem.name().to_owned();
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());
        let c_identifier = elem
            .c_identifier()
            .map(ToOwned::to_owned)
            .unwrap_or(name.clone());
        let mut params = Vec::new();
        let ret = self.read_return_value(ns_id, elem.return_value().clone(), true)?;
        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);

        for param in elem.parameters().inner() {
            let (tid, _c_type, _) =
                self.read_parameter(ns_id, true, param.name(), param.ty().unwrap().clone())?;

            let param = Parameter::Default {
                param: param.clone(),
                tid,
                nullable_override: None,
                name_override: None,
                c_type_override: None,
            };
            params.push(param);
        }

        let throws = elem.throws();
        if throws {
            let tid = self.find_or_stub_type(ns_id, "GLib.Error");
            params.push(Parameter::error(tid));
        }

        Ok(Function {
            name,
            c_identifier,
            kind: FunctionKind::VirtualMethod,
            parameters: params,
            ret,
            throws,
            version,
            deprecated_version,
            doc,
            doc_deprecated,
            get_property: None,
            set_property: None,
            finish_func: None,
            async_func: None,
            sync_func: None,
        })
    }

    fn read_field(&mut self, ns_id: u16, elem: &gir_parser::Field) -> Result<Field, String> {
        let field_name = elem.name();
        let private = elem.is_private();
        let bits = elem.bits();

        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);

        let (tid, c_type, array_length) = match elem.ty() {
            gir_parser::FieldType::Array(array) => self.read_array(ns_id, array),
            gir_parser::FieldType::Callback(callback) => self
                .read_callback(ns_id, callback)
                .map(|f| (Type::function(self, f), None, None)),
            gir_parser::FieldType::Type(ty) => self.read_type(ns_id, ty),
        }?;
        Ok(Field {
            name: field_name.into(),
            typ: tid,
            c_type,
            private,
            bits,
            array_length,
            doc,
        })
    }

    fn read_interface(&mut self, ns_id: u16, elem: &gir_parser::Interface) -> Result<(), String> {
        let name = elem.name();
        let c_type = elem
            .c_type()
            .map(ToOwned::to_owned)
            .ok_or_else(|| format!("<class> {} doesn't have a `c:type` attribute", name))?;
        let symbol_prefix = elem
            .symbol_prefix()
            .map(ToOwned::to_owned)
            .ok_or_else(|| format!("<class> {} doesn't have a `symbol-prefix` attribute", name))?;
        let type_struct = elem.g_type_struct().map(ToOwned::to_owned);
        let get_type = elem.g_get_type();
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());

        let mut fns = Vec::new();
        let mut vfns = Vec::new();
        let mut signals = Vec::new();
        let mut properties = Vec::new();
        let mut prereqs = Vec::new();
        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);

        for constructor in elem.constructors().iter() {
            if constructor.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, constructor, FunctionKind::Constructor)?;
            fns.push(f);
        }
        for method in elem.methods().iter() {
            if method.moved_to().is_some() {
                continue;
            }
            let f = self.read_method(ns_id, method)?;
            fns.push(f);
        }
        for function in elem.functions().iter() {
            if function.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, function, FunctionKind::Function)?;
            fns.push(f);
        }
        for virtual_method in elem.virtual_methods().iter() {
            if virtual_method.moved_to().is_some() {
                continue;
            }
            let f = self.read_virtual_method(ns_id, virtual_method)?;
            vfns.push(f);
        }

        for property in elem.properties().iter() {
            properties.push(self.read_property(ns_id, property, &symbol_prefix)?);
        }

        for signal in elem.signals().iter() {
            let signal = self.read_signal(ns_id, signal)?;
            signals.push(signal);
        }

        for prereq in elem.prerequisites() {
            prereqs.push(self.find_or_stub_type(ns_id, prereq.name()));
        }

        let typ = Type::Interface(Interface {
            name: name.to_owned(),
            c_type,
            type_struct,
            c_class_type: None, // this will be resolved during postprocessing
            glib_get_type: get_type.into(),
            functions: fns,
            virtual_methods: vfns,
            signals,
            properties,
            prerequisites: prereqs,
            doc,
            doc_deprecated,
            version,
            deprecated_version,
            symbol_prefix,
        });
        self.add_type(ns_id, name, typ);
        Ok(())
    }

    fn read_bitfield(&mut self, ns_id: u16, elem: &gir_parser::BitField) -> Result<(), String> {
        let name = elem.name().to_owned();
        let c_type = elem.c_type().to_owned();
        let glib_get_type = elem.g_get_type().map(ToOwned::to_owned);
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());

        let mut members = Vec::new();
        let mut functions = Vec::new();
        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);

        for member in elem.members() {
            let member = self.read_member(ns_id, member);
            members.push(member);
        }

        for function in elem.functions() {
            if function.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, function, FunctionKind::Function)?;
            functions.push(f);
        }

        let typ = Type::Bitfield(Bitfield {
            name: name.clone(),
            c_type,
            members,
            functions,
            version,
            deprecated_version,
            doc,
            doc_deprecated,
            glib_get_type,
        });
        self.add_type(ns_id, &name, typ);
        Ok(())
    }

    fn read_enumeration(
        &mut self,
        ns_id: u16,
        elem: &gir_parser::Enumeration,
    ) -> Result<(), String> {
        let enum_name = elem.name();
        let c_type = elem.c_type();
        let get_type = elem.g_get_type().map(ToOwned::to_owned);
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());
        let error_domain = elem
            .g_error_domain()
            .map(|s| ErrorDomain::Quark(String::from(s)));

        let mut members = Vec::new();
        let mut fns = Vec::new();
        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);

        for member in elem.members() {
            let member = self.read_member(ns_id, member);
            members.push(member);
        }

        for function in elem.functions() {
            if function.moved_to().is_some() {
                continue;
            }
            let f = self.read_function(ns_id, function, FunctionKind::Function)?;
            fns.push(f);
        }

        let typ = Type::Enumeration(Enumeration {
            name: enum_name.into(),
            c_type: c_type.into(),
            members,
            functions: fns,
            version,
            deprecated_version,
            doc,
            doc_deprecated,
            error_domain,
            glib_get_type: get_type,
        });
        self.add_type(ns_id, enum_name, typ);
        Ok(())
    }

    fn read_constant(&mut self, ns_id: u16, elem: &gir_parser::Constant) -> Result<(), String> {
        let name = elem.name();
        let c_identifier = elem.c_type().map(ToOwned::to_owned).ok_or_else(|| {
            format!(
                "<constant> {} element doesn't have a `c:identifier` attribute",
                name
            )
        })?;
        let value = elem.value().to_owned();
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());

        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);

        let (typ, c_type, _array_length) = match elem.ty() {
            gir_parser::AnyType::Type(ty) => self.read_type(ns_id, ty)?,
            gir_parser::AnyType::Array(array) => self.read_array(ns_id, array)?,
        };
        if let Some(c_type) = c_type {
            self.add_constant(
                ns_id,
                Constant {
                    name: name.to_owned(),
                    c_identifier,
                    typ,
                    c_type,
                    value,
                    version,
                    deprecated_version,
                    doc,
                    doc_deprecated,
                },
            );
            Ok(())
        } else {
            Err(String::from(
                "Missing <type> element inside <constant> element",
            ))
        }
    }

    fn read_alias(&mut self, ns_id: u16, elem: &gir_parser::Alias) -> Result<(), String> {
        let alias_name = elem.name();
        let c_identifier = elem.c_type();

        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);

        let (typ, c_type, _) = match elem.ty() {
            gir_parser::AnyType::Type(ty) => self.read_type(ns_id, ty)?,
            gir_parser::AnyType::Array(array) => self.read_array(ns_id, array)?,
        };
        if let Some(c_type) = c_type {
            let typ = Type::Alias(Alias {
                name: alias_name.into(),
                c_identifier: c_identifier.into(),
                typ,
                target_c_type: c_type,
                doc,
                doc_deprecated,
            });
            self.add_type(ns_id, alias_name, typ);
            Ok(())
        } else {
            Err(String::from("Missing <constant> element's c:type"))
        }
    }

    fn read_member(&mut self, ns_id: u16, elem: &gir_parser::Member) -> Member {
        let name = elem.name().to_owned();
        let value = elem.value().to_owned();
        let c_identifier = elem.c_identifier().to_owned();
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());

        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);

        Member {
            name,
            value,
            doc,
            doc_deprecated,
            c_identifier,
            status: crate::config::gobjects::GStatus::Generate,
            version,
            deprecated_version,
        }
    }

    fn read_callback(
        &mut self,
        ns_id: u16,
        elem: &gir_parser::Callback,
    ) -> Result<Function, String> {
        self.read_function_inner(
            ns_id,
            elem,
            FunctionKind::Function,
            elem.parameters(),
            elem.return_value(),
            elem.name(),
            elem.c_type().unwrap_or(""), // Callback don't have a c-identifier??
            None,
            None,
            None,
            None,
            None,
        )
    }

    fn read_method(&mut self, ns_id: u16, elem: &gir_parser::Method) -> Result<Function, String> {
        let gtk_get_property = elem.gtk_method_get_property().map(ToOwned::to_owned);
        let gtk_set_property = elem.gtk_method_set_property().map(ToOwned::to_owned);
        let get_property = gtk_get_property.or(elem.get_property().map(ToString::to_string));
        let set_property = gtk_set_property.or(elem.set_property().map(ToString::to_string));
        self.read_function_inner(
            ns_id,
            elem,
            FunctionKind::Method,
            elem.parameters(),
            elem.return_value(),
            elem.name(),
            elem.c_identifier()
                .ok_or_else(|| format!("method `{}` doesn't have a `c:identifier`", elem.name()))?,
            get_property,
            set_property,
            elem.async_func().map(ToOwned::to_owned),
            elem.sync_func().map(ToOwned::to_owned),
            elem.finish_func().map(ToOwned::to_owned),
        )
    }

    fn read_function(
        &mut self,
        ns_id: u16,
        elem: &gir_parser::Function,
        kind: FunctionKind,
    ) -> Result<Function, String> {
        self.read_function_inner(
            ns_id,
            elem,
            kind,
            elem.parameters(),
            elem.return_value(),
            elem.name(),
            elem.c_identifier().ok_or_else(|| {
                format!("function `{}` doesn't have a `c:identifier`", elem.name())
            })?,
            None,
            None,
            elem.async_func().map(ToOwned::to_owned),
            elem.sync_func().map(ToOwned::to_owned),
            elem.finish_func().map(ToOwned::to_owned),
        )
    }

    fn read_function_inner(
        &mut self,
        ns_id: u16,
        elem: &impl gir_parser::prelude::FunctionLike,
        kind: FunctionKind,
        parameters: &gir_parser::Parameters,
        return_value: &gir_parser::ReturnValue,
        fn_name: &str,
        c_identifier: &str,
        get_property: Option<String>,
        set_property: Option<String>,
        async_func: Option<String>,
        sync_func: Option<String>,
        finish_func: Option<String>,
    ) -> Result<Function, String> {
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());
        let finish_func = finish_func.map(|finish_func_name| {
            format!(
                "{}{finish_func_name}",
                c_identifier.strip_suffix(&fn_name).unwrap()
            )
        });

        let mut params = Vec::new();
        let ret = self.read_return_value(ns_id, return_value.clone(), false)?;
        let doc: Option<String> = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);

        if let Some(instance_param) = parameters.instance() {
            let param = self.read_instance_parameter(ns_id, instance_param.clone(), false)?;
            params.push(param);
        }

        for param in parameters.inner() {
            let ty = if param.name() == "..." {
                gir_parser::ParameterType::VarArgs
            } else {
                param.ty().unwrap().clone()
            };

            let (tid, _c_type, _) = self.read_parameter(ns_id, true, param.name(), ty)?;

            let param = Parameter::Default {
                param: param.clone(),
                tid,
                nullable_override: None,
                name_override: None,
                c_type_override: None,
            };
            params.push(param);
        }

        let throws = elem.throws();
        if throws {
            let tid = self.find_or_stub_type(ns_id, "GLib.Error");
            params.push(Parameter::error(tid));
        }
        Ok(Function {
            name: fn_name.to_owned(),
            c_identifier: c_identifier.to_owned(),
            kind,
            parameters: params,
            ret,
            throws,
            version,
            deprecated_version,
            doc,
            doc_deprecated,
            get_property,
            set_property,
            finish_func,
            async_func,
            sync_func,
        })
    }

    fn read_signal(&mut self, ns_id: u16, elem: &gir_parser::Signal) -> Result<Signal, String> {
        let signal_name = elem.name();
        let is_action = elem.is_action();
        let is_detailed = elem.is_detailed();
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());

        let mut params = Vec::new();
        let ret = self.read_return_value(ns_id, elem.return_value().clone(), true)?;
        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);

        for param in elem.parameters().inner() {
            let (tid, c_type, _) =
                self.read_parameter(ns_id, true, param.name(), param.ty().unwrap().clone())?;
            let param = Parameter::Default {
                param: param.clone(),
                tid,
                nullable_override: None,
                name_override: None,
                c_type_override: Some(c_type),
            };
            params.push(param);
        }

        Ok(Signal {
            name: signal_name.into(),
            parameters: params,
            ret,
            is_action,
            is_detailed,
            version,
            deprecated_version,
            doc,
            doc_deprecated,
        })
    }

    fn read_instance_parameter(
        &mut self,
        ns_id: u16,
        elem: gir_parser::InstanceParameter,
        allow_no_ctype: bool,
    ) -> Result<Parameter, String> {
        let (tid, c_type, _) = self.read_parameter(
            ns_id,
            allow_no_ctype,
            elem.name(),
            elem.ty().unwrap().clone().into(),
        )?;
        Ok(Parameter::Instance {
            param: elem,
            tid,
            c_type_override: Some(c_type),
            name_override: None,
            nullable_override: None,
        })
    }

    fn read_return_value(
        &mut self,
        ns_id: u16,
        elem: gir_parser::ReturnValue,
        _allow_no_ctype: bool,
    ) -> Result<Parameter, String> {
        let (tid, c_type, _) = self
            .read_parameter(ns_id, true, "return-value", elem.ty().clone().into())
            .inspect_err(|_| {
                warn!("Failed to parse {:#?}", elem);
            })?;
        Ok(Parameter::Return {
            param: elem,
            tid,
            c_type_override: Some(c_type),
            name_override: None,
            nullable_override: None,
        })
    }

    fn read_parameter(
        &mut self,
        ns_id: u16,
        allow_no_ctype: bool,
        name: &str,
        ty: gir_parser::ParameterType,
    ) -> Result<(TypeId, String, bool), String> {
        match ty {
            // Safe to unwrap as params without a type are specific macros
            gir_parser::ParameterType::Array(array) => {
                let (tid, c_type, _) = self.read_array(ns_id, &array)?;
                let c_type = c_type
                    .or_else(|| allow_no_ctype.then_some(EMPTY_CTYPE.to_owned()))
                    .ok_or_else(|| format!("Missing c:type attribute in <{}> element", name))?;
                Ok((tid, c_type, false))
            }
            gir_parser::ParameterType::Type(ty) => {
                let (tid, c_type, _) = self.read_type(ns_id, &ty)?;
                let c_type = c_type
                    .or_else(|| allow_no_ctype.then_some(EMPTY_CTYPE.to_owned()))
                    .ok_or_else(|| format!("Missing c:type attribute in <{}> element", name))?;
                Ok((tid, c_type, false))
            }
            gir_parser::ParameterType::VarArgs => {
                let tid = self.find_type(INTERNAL_NAMESPACE, "varargs").unwrap();
                Ok((tid, "varargs".to_owned(), true))
            }
        }
    }

    fn read_property(
        &mut self,
        ns_id: u16,
        elem: &gir_parser::Property,
        symbol_prefix: &str,
    ) -> Result<Property, String> {
        let prop_name = elem.name();
        let readable = elem.is_readable();
        let writable = elem.is_writable();
        let construct = elem.is_construct();
        let construct_only = elem.is_construct_only();
        let transfer = elem.transfer_ownership();
        let version = self.read_version(ns_id, elem.version());
        let deprecated_version = self.read_version(ns_id, elem.deprecated_version());
        let doc = elem.doc().map(|d| d.text()).map(ToOwned::to_owned);
        let doc_deprecated = elem
            .doc_deprecated()
            .map(|d| d.text())
            .map(ToOwned::to_owned);
        let gtk_getter = elem.gtk_property_get().and_then(|p| {
            p.split(symbol_prefix)
                .last()
                .and_then(|p| p.strip_prefix('_'))
                .map(|p| p.to_string())
        });
        let gtk_setter = elem.gtk_property_set().and_then(|p| {
            p.split(symbol_prefix)
                .last()
                .and_then(|p| p.strip_prefix('_'))
                .map(|p| p.to_string())
        });
        let getter = gtk_getter.or(elem.getter().map(ToString::to_string));
        let setter = gtk_setter.or(elem.setter().map(ToString::to_string));
        let (tid, c_type, _) = match elem.ty() {
            gir_parser::AnyType::Array(array) => self.read_array(ns_id, array)?,
            gir_parser::AnyType::Type(ty) => self.read_type(ns_id, ty)?,
        };
        Ok(Property {
            name: prop_name.into(),
            readable,
            writable,
            construct,
            construct_only,
            transfer,
            typ: tid,
            c_type,
            version,
            deprecated_version,
            doc,
            doc_deprecated,
            getter,
            setter,
        })
    }

    fn read_array(
        &mut self,
        ns_id: u16,
        elem: &gir_parser::Array,
    ) -> Result<(TypeId, Option<String>, Option<u32>), String> {
        let type_name = elem.name().unwrap_or("array");
        let array_length = elem.length();

        let tid = if type_name == "array" {
            trace!("Trying to find type {type_name}, array={:#?}", elem);
            let (tid, c_type, _) = self.read_type(ns_id, elem.ty())?;
            Type::c_array(self, tid, elem.fixed_size(), c_type)
        } else if type_name == "GLib.ByteArray" {
            self.find_or_stub_type(ns_id, type_name)
        } else {
            let inner = elem.ty();
            let (inner_ty, _c_type, _) = self.read_type(ns_id, inner)?;
            Type::container(self, type_name, vec![inner_ty])
                .ok_or_else(|| format!("Unknown container type {type_name} {:?}", elem.c_type()))?
        };
        Ok((tid, elem.c_type().map(ToOwned::to_owned), array_length))
    }

    fn read_type(
        &mut self,
        ns_id: u16,
        elem: &gir_parser::Type,
    ) -> Result<(TypeId, Option<String>, Option<u32>), String> {
        let type_name = elem.name().unwrap_or(""); // TODO: should this warn?
        let c_type = elem.c_type().map(ToOwned::to_owned);

        if type_name == "gboolean" && c_type.as_deref() == Some("_Bool") {
            Ok((self.find_or_stub_type(ns_id, "bool"), c_type, None))
        } else {
            Ok((self.find_or_stub_type(ns_id, type_name), c_type, None))
        }
    }

    fn read_version(
        &mut self,
        ns_id: u16,
        version: Option<&gir_parser::Version>,
    ) -> Option<Version> {
        let version = Version::new(
            version?.major(),
            version?.minor().unwrap_or_default(),
            version?.patch().unwrap_or_default(),
        );
        self.register_version(ns_id, version);
        Some(version)
    }
}

fn make_file_name(dir: &Path, name: &str) -> PathBuf {
    let mut path = dir.to_path_buf();
    let name = format!("{name}.gir");
    path.push(name);
    path
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str::FromStr};

    use super::*;

    #[test]
    fn test_pango_test_gir() {
        let content = br#"<?xml version="1.0"?>
<repository xmlns="http://www.gtk.org/introspection/core/1.0" xmlns:c="http://www.gtk.org/introspection/c/1.0" xmlns:doc="http://www.gtk.org/introspection/doc/1.0" xmlns:glib="http://www.gtk.org/introspection/glib/1.0" version="1.2">
  <include name="GObject" version="2.0"/>
  <package name="pango"/>
  <c:include name="pango/pango.h"/>
  <doc:format name="gi-docgen"/>
  <namespace name="Pango" version="1.0" shared-library="libpango-1.0.so.0" c:identifier-prefixes="Pango" c:symbol-prefixes="pango">
    <class name="Context" c:symbol-prefix="context" c:type="PangoContext" parent="GObject.Object" glib:type-name="PangoContext" glib:get-type="pango_context_get_type" glib:type-struct="ContextClass">
      <method name="list_families" c:identifier="pango_context_list_families">
        <return-value transfer-ownership="none">
          <type name="none" c:type="void"/>
        </return-value>
        <parameters>
          <instance-parameter name="context" transfer-ownership="none">
            <type name="Context" c:type="PangoContext*"/>
          </instance-parameter>
          <parameter name="families" direction="out" caller-allocates="0" transfer-ownership="container">
            <array length="1" zero-terminated="0" c:type="PangoFontFamily***">
              <type name="FontFamily" c:type="PangoFontFamily**"/>
            </array>
          </parameter>
          <parameter name="n_families" direction="out" caller-allocates="0" transfer-ownership="full">
            <type name="gint" c:type="int*"/>
          </parameter>
        </parameters>
      </method>
    </class>
    <class name="FontFamily" c:symbol-prefix="font_family" c:type="PangoFontFamily" parent="GObject.Object" abstract="1" glib:type-name="PangoFontFamily" glib:get-type="pango_font_family_get_type" glib:type-struct="FontFamilyClass">
    </class>
  </namespace>
</repository>"#;
        let mut lib = crate::Library::new("Pango");
        let mut parser = crate::xmlparser::XmlParser::new(&content[..]);
        let dirs = vec!["../gir-files"];
        let mut libs = vec!["Pango".to_string()];
        parser.document(|p, _| {
            p.element_with_name("repository", |sub_parser, _elem| {
                lib.read_repository(&dirs, sub_parser, &mut libs)
            })
        });

        const PANGO_NS_ID: u16 = 1;
        let expected_index = HashMap::from([
            ("*".to_string(), 0),
            ("Pango".to_string(), PANGO_NS_ID),
            ("GLib".to_string(), 2),
            ("GObject".to_string(), 3),
        ]);

        assert_eq!(&lib.index, &expected_index);
        assert_eq!(lib.doc_format, DocFormat::GiDocgen);

        let pango_ns = &lib.namespaces[PANGO_NS_ID as usize];
        assert_eq!(pango_ns.types.len(), 2);

        let Some(crate::parser::Type::Class(context_class)) = &pango_ns.types[0] else {
            panic!();
        };
        assert_eq!(context_class.name, "Context");

        let Some(crate::parser::Type::Class(font_family_class)) = &pango_ns.types[1] else {
            panic!();
        };
        assert_eq!(font_family_class.name, "FontFamily");

        assert_eq!(context_class.functions.len(), 1);
        let list_families = &context_class.functions[0];
        assert_eq!(list_families.name, "list_families");

        if let crate::parser::Parameter { typ, .. } = list_families.ret {
            assert_eq!(typ, TypeId::tid_none());
        } else {
            panic!();
        };

        assert_eq!(list_families.parameters.len(), 3);

        let context_par = &list_families.parameters[0];
        let families_par = &list_families.parameters[1];
        let n_families_par = &list_families.parameters[2];

        if let crate::parser::Parameter { typ, .. } = context_par {
            assert_eq!(
                typ,
                &TypeId {
                    ns_id: PANGO_NS_ID,
                    id: 0
                }
            );
        } else {
            panic!()
        };
        if let crate::parser::Parameter { typ, .. } = families_par {
            assert_eq!(typ, &TypeId { ns_id: 0, id: 128 });
        } else {
            panic!()
        };
        if let crate::parser::Parameter { typ, .. } = n_families_par {
            assert_eq!(typ, &TypeId { ns_id: 0, id: 14 });
        } else {
            panic!()
        };
    }
}
