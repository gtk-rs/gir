use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use log::{trace, warn};

use crate::{
    library::*,
    version::Version,
    xmlparser::{Element, XmlParser},
};

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
        for dir in dirs {
            let dir: &Path = dir.as_ref();
            let file_name = make_file_name(dir, &libs[libs.len() - 1]);
            let mut parser = match XmlParser::from_path(&file_name) {
                Ok(p) => p,
                _ => continue,
            };
            return parser.document(|p, _| {
                p.element_with_name("repository", |sub_parser, _elem| {
                    self.read_repository(dirs, sub_parser, libs)
                })
            });
        }
        Err(format!("Couldn't find `{}`...", &libs[libs.len() - 1]))
    }

    fn read_repository<P: AsRef<Path>>(
        &mut self,
        dirs: &[P],
        parser: &mut XmlParser<'_>,
        libs: &mut Vec<String>,
    ) -> Result<(), String> {
        let mut packages = Vec::new();
        let mut includes = Vec::new();
        parser.elements(|parser, elem| match elem.name() {
            "include" => {
                match (elem.attr("name"), elem.attr("version")) {
                    (Some(name), Some(ver)) => {
                        if self.find_namespace(name).is_none() {
                            let lib = format!("{name}-{ver}");
                            if libs.iter().any(|x| *x == lib) {
                                return Err(format!(
                                    "`{}` includes itself (full path:`{}`)!",
                                    lib,
                                    libs.join("::")
                                ));
                            }
                            libs.push(lib);
                            self.read_file(dirs, libs)?;
                            libs.pop();
                        }
                    }
                    (Some(name), None) => includes.push(name.to_owned()),
                    _ => {}
                }
                Ok(())
            }
            "package" => {
                let name = elem.attr_required("name")?;
                packages.push(name.to_owned());
                Ok(())
            }
            "namespace" => self.read_namespace(
                parser,
                elem,
                std::mem::take(&mut packages),
                std::mem::take(&mut includes),
            ),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;
        Ok(())
    }

    fn read_namespace(
        &mut self,
        parser: &mut XmlParser<'_>,
        elem: &Element,
        packages: Vec<String>,
        c_includes: Vec<String>,
    ) -> Result<(), String> {
        let ns_name = elem.attr_required("name")?;
        let ns_id = self.add_namespace(ns_name);

        {
            let ns = self.namespace_mut(ns_id);
            ns.package_names = packages;
            ns.c_includes = c_includes;
            if let Some(s) = elem.attr("shared-library") {
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
            if let Some(s) = elem.attr("identifier-prefixes") {
                ns.identifier_prefixes = s.split(',').map(String::from).collect();
            }
            if let Some(s) = elem.attr("symbol-prefixes") {
                ns.symbol_prefixes = s.split(',').map(String::from).collect();
            }
        }

        trace!(
            "Reading {}-{}",
            ns_name,
            elem.attr("version").unwrap_or("?")
        );

        parser.elements(|parser, elem| {
            trace!("<{} name={:?}>", elem.name(), elem.attr("name"));
            match elem.name() {
                "class" => self.read_class(parser, ns_id, elem),
                "record" => self.read_record_start(parser, ns_id, elem),
                "union" => self.read_named_union(parser, ns_id, elem),
                "interface" => self.read_interface(parser, ns_id, elem),
                "callback" => self.read_named_callback(parser, ns_id, elem),
                "bitfield" => self.read_bitfield(parser, ns_id, elem),
                "enumeration" => self.read_enumeration(parser, ns_id, elem),
                "function" => self.read_global_function(parser, ns_id, elem),
                "constant" => self.read_constant(parser, ns_id, elem),
                "alias" => self.read_alias(parser, ns_id, elem),
                "boxed" | "function-macro" | "docsection" => parser.ignore_element(),
                _ => {
                    warn!("<{} name={:?}>", elem.name(), elem.attr("name"));
                    parser.ignore_element()
                }
            }
        })?;
        Ok(())
    }

    fn read_class(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        let class_name = elem.attr_required("name")?;
        let c_type = self.read_object_c_type(parser, elem)?;
        let symbol_prefix = elem.attr_required("symbol-prefix").map(ToOwned::to_owned)?;
        let type_struct = elem.attr("type-struct").map(ToOwned::to_owned);
        let get_type = elem.attr_required("get-type")?;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let is_fundamental = elem.attr("fundamental").map_or(false, |x| x == "1");
        let (ref_fn, unref_fn) = if is_fundamental {
            (
                elem.attr("ref-func").map(ToOwned::to_owned),
                elem.attr("unref-func").map(ToOwned::to_owned),
            )
        } else {
            (None, None)
        };

        let is_abstract = elem.attr("abstract").map_or(false, |x| x == "1");

        let mut fns = Vec::new();
        let mut signals = Vec::new();
        let mut properties = Vec::new();
        let mut impls = Vec::new();
        let mut fields = Vec::new();
        let mut vfns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;
        let mut union_count = 1;

        parser.elements(|parser, elem| match elem.name() {
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            }
            "implements" => self.read_type(parser, ns_id, elem).map(|r| {
                impls.push(r.0);
            }),
            "signal" => self
                .read_signal(parser, ns_id, elem)
                .map(|s| signals.push(s)),
            "property" => self.read_property(parser, ns_id, elem).map(|p| {
                if let Some(p) = p {
                    properties.push(p);
                }
            }),
            "field" => self.read_field(parser, ns_id, elem).map(|f| {
                fields.push(f);
            }),
            "virtual-method" => self
                .read_virtual_method(parser, ns_id, elem)
                .map(|v| vfns.push(v)),
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "source-position" => parser.ignore_element(),
            "union" => self
                .read_union(parser, ns_id, elem, Some(class_name), Some(c_type))
                .map(|mut u| {
                    let field_name = if let Some(field_name) = elem.attr("name") {
                        field_name.into()
                    } else {
                        format!("u{union_count}")
                    };

                    u = Union {
                        name: format!("{class_name}_{field_name}"),
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
                }),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        let parent = elem
            .attr("parent")
            .map(|s| self.find_or_stub_type(ns_id, s));
        let typ = Type::Class(Class {
            name: class_name.into(),
            c_type: c_type.into(),
            type_struct,
            c_class_type: None, // this will be resolved during postprocessing
            glib_get_type: get_type.into(),
            fields,
            functions: fns,
            virtual_methods: vfns,
            signals,
            properties,
            parent,
            implements: impls,
            final_type: false, // this will be set during postprocessing
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
        self.add_type(ns_id, class_name, typ);
        Ok(())
    }

    fn read_record_start(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        if let Some(typ) = self.read_record(parser, ns_id, elem, None, None)? {
            let name = typ.get_name();
            self.add_type(ns_id, &name, typ);
        }
        Ok(())
    }

    fn read_record(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
        parent_name_prefix: Option<&str>,
        parent_ctype_prefix: Option<&str>,
    ) -> Result<Option<Type>, String> {
        let record_name = elem.attr_required("name")?;
        // Records starting with `_` are intended to be private and should not be bound
        if record_name.starts_with('_') {
            parser.ignore_element()?;
            return Ok(None);
        }
        let is_class_record = record_name.ends_with("Class");

        let c_type = elem.attr_required("type")?;
        let symbol_prefix = elem.attr("symbol-prefix").map(ToOwned::to_owned);
        let get_type = elem.attr("get-type").map(ToOwned::to_owned);
        let gtype_struct_for = elem.attr("is-gtype-struct-for");
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let disguised = elem.attr_bool("disguised", false);

        let mut fields = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;
        let mut union_count = 1;

        parser.elements(|parser, elem| match elem.name() {
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            }
            "union" => self
                .read_union(parser, ns_id, elem, Some(record_name), Some(c_type))
                .map(|mut u| {
                    let field_name = if let Some(field_name) = elem.attr("name") {
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
                }),
            "field" => {
                self.read_field(parser, ns_id, elem).map(|mut f| {
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
                        return;
                    }
                    // Workaround for wrong GValue c:type
                    if c_type == "GValue" && f.name == "data" {
                        f.c_type = Some("GValue_data".into());
                    }
                    fields.push(f);
                })
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "source-position" => parser.ignore_element(),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

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
            symbol_prefix,
        });

        Ok(Some(typ))
    }

    fn read_named_union(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        // Require a name here
        elem.attr_required("name")?;

        self.read_union(parser, ns_id, elem, None, None)
            .and_then(|mut u| {
                assert_ne!(u.name, "");
                // Workaround for missing c:type
                if u.name == "_Value__data__union" {
                    u.c_type = Some("GValue_data".into());
                } else if u.c_type.is_none() {
                    return Err(parser.fail("Missing union c:type"));
                }
                let union_name = u.name.clone();
                self.add_type(ns_id, &union_name, Type::Union(u));
                Ok(())
            })
    }

    fn read_union(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
        parent_name_prefix: Option<&str>,
        parent_ctype_prefix: Option<&str>,
    ) -> Result<Union, String> {
        let union_name = elem.attr("name").unwrap_or("");
        let c_type = self.read_object_c_type(parser, elem).unwrap_or("");
        let get_type = elem.attr("get-type").map(|s| s.into());
        let symbol_prefix = elem.attr("symbol-prefix").map(ToOwned::to_owned);

        let mut fields = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut struct_count = 1;

        parser.elements(|parser, elem| match elem.name() {
            "source-position" => parser.ignore_element(),
            "field" => self.read_field(parser, ns_id, elem).map(|f| {
                fields.push(f);
            }),
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            }
            "record" => {
                let mut r = match self.read_record(
                    parser,
                    ns_id,
                    elem,
                    parent_name_prefix,
                    parent_ctype_prefix,
                )? {
                    Some(Type::Record(r)) => r,
                    _ => return Ok(()),
                };

                let field_name = if let Some(field_name) = elem.attr("name") {
                    field_name.into()
                } else {
                    format!("s{struct_count}")
                };

                r = Record {
                    name: format!(
                        "{}{}_{}",
                        parent_name_prefix.map_or_else(String::new, |s| { format!("{s}_") }),
                        union_name,
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

                Ok(())
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        Ok(Union {
            name: union_name.into(),
            c_type: Some(c_type.into()),
            glib_get_type: get_type,
            fields,
            functions: fns,
            doc,
            symbol_prefix,
        })
    }

    fn read_virtual_method(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
    ) -> Result<Function, String> {
        let method_name = elem.attr_required("name")?;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let c_identifier = elem.attr("identifier").or_else(|| elem.attr("name"));
        let mut params = Vec::new();
        let mut ret = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "parameters" => self
                .read_parameters(parser, ns_id, true, true)
                .map(|mut ps| params.append(&mut ps)),
            "return-value" => {
                if ret.is_some() {
                    return Err(parser.fail("Too many <return-value> elements"));
                }
                self.read_parameter(parser, ns_id, elem, true, false)
                    .map(|p| ret = Some(p))
            }
            "source-position" => parser.ignore_element(),
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        let throws = elem.attr_bool("throws", false);
        if throws {
            params.push(Parameter {
                name: "error".into(),
                typ: self.find_or_stub_type(ns_id, "GLib.Error"),
                c_type: "GError**".into(),
                instance_parameter: false,
                direction: ParameterDirection::Out,
                transfer: Transfer::Full,
                caller_allocates: false,
                nullable: Nullable(true),
                array_length: None,
                is_error: true,
                doc: None,
                scope: ParameterScope::None,
                closure: None,
                destroy: None,
            });
        }

        if let Some(ret) = ret {
            Ok(Function {
                name: method_name.into(),
                c_identifier: c_identifier.map(|s| s.into()),
                kind: FunctionKind::VirtualMethod,
                parameters: params,
                ret,
                throws,
                version,
                deprecated_version,
                doc,
                doc_deprecated,
            })
        } else {
            Err(parser.fail("Missing <return-value> element"))
        }
    }

    fn read_field(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<Field, String> {
        let field_name = elem.attr_required("name")?;
        let private = elem.attr_bool("private", false);
        let bits = elem.attr("bits").and_then(|s| s.parse().ok());

        let mut typ = None;
        let mut doc = None;

        parser.elements(|parser, elem| match elem.name() {
            "type" | "array" => {
                if typ.is_some() {
                    return Err(parser.fail("Too many <type> elements"));
                }
                self.read_type(parser, ns_id, elem).map(|t| {
                    typ = Some(t);
                })
            }
            "callback" => {
                if typ.is_some() {
                    return Err(parser.fail("Too many <type> elements"));
                }
                self.read_function(parser, ns_id, elem.name(), elem, true)
                    .map(|f| {
                        typ = Some((Type::function(self, f), None, None));
                    })
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        if let Some((tid, c_type, array_length)) = typ {
            Ok(Field {
                name: field_name.into(),
                typ: tid,
                c_type,
                private,
                bits,
                array_length,
                doc,
            })
        } else {
            Err(parser.fail("Missing <type> element"))
        }
    }

    fn read_named_callback(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        self.read_function_if_not_moved(parser, ns_id, elem.name(), elem, true)?
            .map(|func| {
                let name = func.name.clone();
                self.add_type(ns_id, &name, Type::Function(func))
            });

        Ok(())
    }

    fn read_interface(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        let interface_name = elem.attr_required("name")?;
        let c_type = self.read_object_c_type(parser, elem)?;
        let symbol_prefix = elem.attr_required("symbol-prefix").map(ToOwned::to_owned)?;
        let type_struct = elem.attr("type-struct").map(ToOwned::to_owned);
        let get_type = elem.attr_required("get-type")?;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut fns = Vec::new();
        let mut vfns = Vec::new();
        let mut signals = Vec::new();
        let mut properties = Vec::new();
        let mut prereqs = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            }
            "prerequisite" => self.read_type(parser, ns_id, elem).map(|r| {
                prereqs.push(r.0);
            }),
            "signal" => self
                .read_signal(parser, ns_id, elem)
                .map(|s| signals.push(s)),
            "property" => self.read_property(parser, ns_id, elem).map(|p| {
                if let Some(p) = p {
                    properties.push(p);
                }
            }),
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "virtual-method" => self
                .read_virtual_method(parser, ns_id, elem)
                .map(|v| vfns.push(v)),
            "source-position" => parser.ignore_element(),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        let typ = Type::Interface(Interface {
            name: interface_name.into(),
            c_type: c_type.into(),
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
        self.add_type(ns_id, interface_name, typ);
        Ok(())
    }

    fn read_bitfield(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        let bitfield_name = elem.attr_required("name")?;
        let c_type = self.read_object_c_type(parser, elem)?;
        let symbol_prefix = elem.attr("symbol-prefix").map(ToOwned::to_owned);
        let get_type = elem.attr("get-type").map(|s| s.into());
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut members = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "member" => self
                .read_member(parser, ns_id, elem)
                .map(|m| members.push(m)),
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "source-position" => parser.ignore_element(),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        let typ = Type::Bitfield(Bitfield {
            name: bitfield_name.into(),
            c_type: c_type.into(),
            members,
            functions: fns,
            version,
            deprecated_version,
            doc,
            doc_deprecated,
            glib_get_type: get_type,
            symbol_prefix,
        });
        self.add_type(ns_id, bitfield_name, typ);
        Ok(())
    }

    fn read_enumeration(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        let enum_name = elem.attr_required("name")?;
        let c_type = self.read_object_c_type(parser, elem)?;
        let symbol_prefix = elem.attr("symbol-prefix").map(ToOwned::to_owned);
        let get_type = elem.attr("get-type").map(|s| s.into());
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let error_domain = elem
            .attr("error-domain")
            .map(|s| ErrorDomain::Quark(String::from(s)));

        let mut members = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "member" => self
                .read_member(parser, ns_id, elem)
                .map(|m| members.push(m)),
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "source-position" => parser.ignore_element(),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

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
            symbol_prefix,
        });
        self.add_type(ns_id, enum_name, typ);
        Ok(())
    }

    fn read_global_function(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        self.read_function_if_not_moved(parser, ns_id, "global", elem, false)
            .map(|func| {
                if let Some(func) = func {
                    self.add_function(ns_id, func);
                }
            })
    }

    fn read_constant(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        let const_name = elem.attr_required("name")?;
        let c_identifier = elem.attr_required("type")?;
        let value = elem.attr_required("value")?;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut inner = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "type" | "array" => {
                if inner.is_some() {
                    return Err(parser.fail_with_position(
                        "Too many <type> inner elements in <constant> element",
                        elem.position(),
                    ));
                }
                let (typ, c_type, array_length) = self.read_type(parser, ns_id, elem)?;
                if let Some(c_type) = c_type {
                    inner = Some((typ, c_type, array_length));
                } else {
                    return Err(parser.fail_with_position(
                        "Missing <constant> element's c:type",
                        elem.position(),
                    ));
                }
                Ok(())
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "source-position" => parser.ignore_element(),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        if let Some((typ, c_type, _array_length)) = inner {
            self.add_constant(
                ns_id,
                Constant {
                    name: const_name.into(),
                    c_identifier: c_identifier.into(),
                    typ,
                    c_type,
                    value: value.into(),
                    version,
                    deprecated_version,
                    doc,
                    doc_deprecated,
                },
            );
            Ok(())
        } else {
            Err(parser.fail_with_position(
                "Missing <type> element inside <constant> element",
                elem.position(),
            ))
        }
    }

    fn read_alias(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        let alias_name = elem.attr_required("name")?;
        let c_identifier = elem.attr_required("type")?;

        let mut inner = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "source-position" => parser.ignore_element(),
            "type" | "array" => {
                if inner.is_some() {
                    return Err(parser.fail_with_position(
                        "Too many <type> inner elements in <alias> element",
                        elem.position(),
                    ));
                }
                let (typ, c_type, array_length) = self.read_type(parser, ns_id, elem)?;
                if let Some(c_type) = c_type {
                    inner = Some((typ, c_type, array_length));
                } else {
                    return Err(parser.fail("Missing <alias> target's c:type"));
                }
                Ok(())
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        if let Some((typ, c_type, _array_length)) = inner {
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
            Err(parser.fail_with_position(
                "Missing <type> element inside <alias> element",
                elem.position(),
            ))
        }
    }

    fn read_member(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<Member, String> {
        let member_name = elem.attr_required("name")?;
        let value = elem.attr_required("value")?;
        let c_identifier = elem.attr("identifier").map(|x| x.into());
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        Ok(Member {
            name: member_name.into(),
            value: value.into(),
            doc,
            doc_deprecated,
            c_identifier: c_identifier.unwrap_or_else(|| member_name.into()),
            status: crate::config::gobjects::GStatus::Generate,
            version,
            deprecated_version,
        })
    }

    fn read_function(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        kind_str: &str,
        elem: &Element,
        is_callback: bool,
    ) -> Result<Function, String> {
        let fn_name = elem.attr_required("name")?;
        let c_identifier = elem.attr("identifier").or_else(|| elem.attr("type"));
        let kind = FunctionKind::from_str(kind_str).map_err(|why| parser.fail(&why))?;
        let is_method = kind == FunctionKind::Method;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut params = Vec::new();
        let mut ret = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "parameters" => self
                .read_parameters(parser, ns_id, false, is_method)
                .map(|mut ps| params.append(&mut ps)),
            "return-value" => {
                if ret.is_some() {
                    return Err(parser.fail_with_position(
                        "Too many <return-value> elements inside <function> element",
                        elem.position(),
                    ));
                }
                ret = Some(self.read_parameter(parser, ns_id, elem, false, is_method)?);
                Ok(())
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "doc-version" => parser.ignore_element(),
            "source-position" => parser.ignore_element(),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;
        // The last argument of a callback is ALWAYS user data, so it has to be marked as such
        // in case it's missing.
        if is_callback && params.last().map(|x| x.closure.is_none()).unwrap_or(false) {
            params.last_mut().unwrap().closure = Some(2000);
        }

        let throws = elem.attr_bool("throws", false);
        if throws {
            params.push(Parameter {
                name: "error".into(),
                typ: self.find_or_stub_type(ns_id, "GLib.Error"),
                c_type: "GError**".into(),
                instance_parameter: false,
                direction: ParameterDirection::Out,
                transfer: Transfer::Full,
                caller_allocates: false,
                nullable: Nullable(true),
                array_length: None,
                is_error: true,
                doc: None,
                scope: ParameterScope::None,
                closure: None,
                destroy: None,
            });
        }
        if let Some(ret) = ret {
            Ok(Function {
                name: fn_name.into(),
                c_identifier: c_identifier.map(|s| s.into()),
                kind,
                parameters: params,
                ret,
                throws,
                version,
                deprecated_version,
                doc,
                doc_deprecated,
            })
        } else {
            Err(parser.fail_with_position(
                "Missing <return-value> element in <function> element",
                elem.position(),
            ))
        }
    }

    fn read_function_to_vec(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
        fns: &mut Vec<Function>,
    ) -> Result<(), String> {
        if let Some(f) = self.read_function_if_not_moved(parser, ns_id, elem.name(), elem, false)? {
            fns.push(f);
        }
        Ok(())
    }

    fn read_function_if_not_moved(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        kind_str: &str,
        elem: &Element,
        is_callback: bool,
    ) -> Result<Option<Function>, String> {
        if elem.attr("moved-to").is_some() {
            return parser.ignore_element().map(|_| None);
        }
        self.read_function(parser, ns_id, kind_str, elem, is_callback)
            .and_then(|f| {
                if f.c_identifier.is_none() {
                    return Err(parser.fail_with_position(
                        &format!(
                            "Missing c:identifier attribute in <{}> element",
                            elem.name()
                        ),
                        elem.position(),
                    ));
                }
                Ok(Some(f))
            })
    }

    fn read_signal(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<Signal, String> {
        let signal_name = elem.attr_required("name")?;
        let is_action = elem.attr_bool("action", false);
        let is_detailed = elem.attr_bool("detailed", false);
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut params = Vec::new();
        let mut ret = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "parameters" => self
                .read_parameters(parser, ns_id, true, false)
                .map(|mut ps| params.append(&mut ps)),
            "return-value" => {
                if ret.is_some() {
                    return Err(parser.fail_with_position(
                        "Too many <return-value> elements in <signal> element",
                        elem.position(),
                    ));
                }
                self.read_parameter(parser, ns_id, elem, true, false)
                    .map(|p| ret = Some(p))
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;
        if let Some(ret) = ret {
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
        } else {
            Err(parser.fail_with_position(
                "Missing <return-value> element in <signal> element",
                elem.position(),
            ))
        }
    }

    fn read_parameters(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        allow_no_ctype: bool,
        for_method: bool,
    ) -> Result<Vec<Parameter>, String> {
        parser.elements(|parser, elem| match elem.name() {
            "parameter" | "instance-parameter" => {
                self.read_parameter(parser, ns_id, elem, allow_no_ctype, for_method)
            }
            _ => Err(parser.unexpected_element(elem)),
        })
    }

    fn read_parameter(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
        allow_no_ctype: bool,
        for_method: bool,
    ) -> Result<Parameter, String> {
        let param_name = elem.attr("name").unwrap_or("");
        let instance_parameter = elem.name() == "instance-parameter";
        let transfer = elem
            .attr_from_str("transfer-ownership")?
            .unwrap_or(Transfer::None);
        let nullable = elem.attr_bool("nullable", false);
        let scope = elem.attr_from_str("scope")?.unwrap_or(ParameterScope::None);
        let closure = elem.attr_from_str("closure")?;
        let destroy = elem.attr_from_str("destroy")?;
        let caller_allocates = elem.attr_bool("caller-allocates", false);
        let direction = if elem.name() == "return-value" {
            Ok(ParameterDirection::Return)
        } else {
            ParameterDirection::from_str(elem.attr("direction").unwrap_or("in"))
                .map_err(|why| parser.fail_with_position(&why, elem.position()))
        }?;

        let mut typ = None;
        let mut varargs = false;
        let mut doc = None;

        parser.elements(|parser, elem| match elem.name() {
            "type" | "array" => {
                if typ.is_some() {
                    return Err(parser.fail_with_position(
                        &format!("Too many <type> elements in <{}> element", elem.name()),
                        elem.position(),
                    ));
                }
                typ = Some(self.read_type(parser, ns_id, elem)?);
                if let Some((tid, None, _)) = typ {
                    if allow_no_ctype {
                        typ = Some((tid, Some(EMPTY_CTYPE.to_owned()), None));
                    } else {
                        return Err(parser.fail_with_position(
                            &format!("Missing c:type attribute in <{}> element", elem.name()),
                            elem.position(),
                        ));
                    }
                }
                Ok(())
            }
            "varargs" => {
                varargs = true;
                parser.ignore_element()
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        if let Some((tid, c_type, mut array_length)) = typ {
            if for_method {
                array_length = array_length.map(|l| l + 1);
            }
            Ok(Parameter {
                name: param_name.into(),
                typ: tid,
                c_type: c_type.unwrap(),
                instance_parameter,
                direction,
                transfer,
                caller_allocates,
                nullable: Nullable(nullable),
                array_length,
                is_error: false,
                doc,
                scope,
                closure,
                destroy,
            })
        } else if varargs {
            Ok(Parameter {
                name: String::new(),
                typ: self.find_type(INTERNAL_NAMESPACE, "varargs").unwrap(),
                c_type: String::new(),
                instance_parameter,
                direction: Default::default(),
                transfer: Transfer::None,
                caller_allocates: false,
                nullable: Nullable(false),
                array_length: None,
                is_error: false,
                doc,
                scope,
                closure,
                destroy,
            })
        } else {
            Err(parser.fail_with_position(
                &format!("Missing <type> element in <{}> element", elem.name()),
                elem.position(),
            ))
        }
    }

    fn read_property(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<Option<Property>, String> {
        let prop_name = elem.attr_required("name")?;
        let readable = elem.attr_bool("readable", true);
        let writable = elem.attr_bool("writable", false);
        let construct = elem.attr_bool("construct", false);
        let construct_only = elem.attr_bool("construct-only", false);
        let transfer = Transfer::from_str(elem.attr("transfer-ownership").unwrap_or("none"))
            .map_err(|why| parser.fail_with_position(&why, elem.position()))?;

        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let mut has_empty_type_tag = false;
        let mut typ = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "type" | "array" => {
                if typ.is_some() {
                    return Err(parser.fail_with_position(
                        "Too many <type> elements in <property> element",
                        elem.position(),
                    ));
                }
                if !elem.has_attrs() && elem.name() == "type" {
                    // defend from <type/>
                    has_empty_type_tag = true;
                    return parser.ignore_element();
                }
                typ = Some(self.read_type(parser, ns_id, elem)?);
                if let Some((tid, None, _)) = typ {
                    typ = Some((tid, Some(EMPTY_CTYPE.to_owned()), None));
                }
                Ok(())
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "attribute" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        if has_empty_type_tag {
            return Ok(None);
        }

        if let Some((tid, c_type, _array_length)) = typ {
            Ok(Some(Property {
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
            }))
        } else {
            Err(parser.fail_with_position(
                "Missing <type> element in <property> element",
                elem.position(),
            ))
        }
    }

    fn read_type(
        &mut self,
        parser: &mut XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(TypeId, Option<String>, Option<u32>), String> {
        let type_name = elem
            .attr("name")
            .or_else(|| {
                if elem.name() == "array" {
                    Some("array")
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                parser.fail_with_position(
                    "<type> element is missing a name attribute",
                    elem.position(),
                )
            })?;
        let c_type = elem.attr("type").map(|s| s.into());
        let array_length = elem.attr("length").and_then(|s| s.parse().ok());

        let inner = parser.elements(|parser, elem| match elem.name() {
            "type" | "array" => self.read_type(parser, ns_id, elem),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        if inner.is_empty() || type_name == "GLib.ByteArray" {
            if type_name == "array" {
                Err(parser.fail_with_position(
                    "<type> element is missing an inner element type",
                    elem.position(),
                ))
            } else if type_name == "gboolean" && c_type.as_deref() == Some("_Bool") {
                Ok((self.find_or_stub_type(ns_id, "bool"), c_type, array_length))
            } else {
                Ok((
                    self.find_or_stub_type(ns_id, type_name),
                    c_type,
                    array_length,
                ))
            }
        } else {
            let tid = if type_name == "array" {
                let inner_type = &inner[0];
                Type::c_array(
                    self,
                    inner_type.0,
                    elem.attr("fixed-size").and_then(|n| n.parse().ok()),
                    inner_type.1.clone(),
                )
            } else {
                let inner = inner.iter().map(|r| r.0).collect();
                Type::container(self, type_name, inner).ok_or_else(|| {
                    parser.fail_with_position("Unknown container type", elem.position())
                })?
            };
            Ok((tid, c_type, array_length))
        }
    }

    fn read_version(
        &mut self,
        parser: &XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<Option<Version>, String> {
        self.read_version_attribute(parser, ns_id, elem, "version")
    }

    fn read_deprecated_version(
        &mut self,
        parser: &XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
    ) -> Result<Option<Version>, String> {
        self.read_version_attribute(parser, ns_id, elem, "deprecated-version")
    }

    fn read_version_attribute(
        &mut self,
        parser: &XmlParser<'_>,
        ns_id: u16,
        elem: &Element,
        attr: &str,
    ) -> Result<Option<Version>, String> {
        if let Some(v) = elem.attr(attr) {
            match v.parse() {
                Ok(v) => {
                    self.register_version(ns_id, v);
                    Ok(Some(v))
                }
                Err(e) => Err(parser.fail(&format!("Invalid `{attr}` attribute: {e}"))),
            }
        } else {
            Ok(None)
        }
    }

    fn read_object_c_type<'a>(
        &mut self,
        parser: &mut XmlParser<'_>,
        elem: &'a Element,
    ) -> Result<&'a str, String> {
        elem.attr("type")
            .or_else(|| elem.attr("type-name"))
            .ok_or_else(|| {
                parser.fail(&format!(
                    "Missing `c:type`/`glib:type-name` attributes on element <{}>",
                    elem.name()
                ))
            })
    }
}

fn make_file_name(dir: &Path, name: &str) -> PathBuf {
    let mut path = dir.to_path_buf();
    let name = format!("{name}.gir");
    path.push(name);
    path
}
