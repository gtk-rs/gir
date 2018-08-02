use std::io::Read;
use std::mem::replace;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use library::*;
use version::Version;
use xmlparser::{Element, XmlParser};

pub const EMPTY_CTYPE: &str = "/*EMPTY*/";

pub fn is_empty_c_type(c_type: &str) -> bool {
    c_type == EMPTY_CTYPE
}

impl Library {
    pub fn read_file(&mut self, dir: &Path, lib: &str) -> Result<(), String> {
        let file_name = make_file_name(dir, lib);
        let mut p = XmlParser::from_path(&file_name)?;
        p.document(|p, _| {
            p.element_with_name("repository", |parser, _elem| {
                self.read_repository(dir, parser, false)
            })
        })
    }

    pub fn read_reader<'a, R: Read, P: Into<Option<&'a Path>>>(
        &mut self,
        reader: R,
        dir: P,
    ) -> Result<(), String> {
        let dir = if let Some(dir) = dir.into() {
            dir
        } else {
            Path::new("directory for include not passed into read_reader")
        };
        let mut p = XmlParser::new(reader)?;
        p.document(|p, _| {
            p.element_with_name("repository", |parser, _elem| {
                self.read_repository(dir, parser, true)
            })
        })
    }

    fn read_repository(
        &mut self,
        dir: &Path,
        parser: &mut XmlParser,
        include_existing: bool,
    ) -> Result<(), String> {
        let mut package = None;
        let mut includes = Vec::new();
        parser.elements(|parser, elem| match elem.name() {
            "include" => {
                match (elem.attr("name"), elem.attr("version")) {
                    (Some(name), Some(ver)) => {
                        if include_existing || self.find_namespace(name).is_none() {
                            let lib = format!("{}-{}", name, ver);
                            self.read_file(dir, &lib)?;
                        }
                    }
                    (Some(name), None) => includes.push(name.to_owned()),
                    _ => {}
                }
                Ok(())
            }
            "package" => {
                // Take the first package element and ignore any other ones.
                if package.is_none() {
                    let name = elem.attr_required("name")?;
                    package = Some(name.to_owned());
                }
                Ok(())
            }
            "namespace" => self.read_namespace(
                parser,
                elem,
                package.take(),
                replace(&mut includes, Vec::new()),
            ),
            _ => Err(parser.unexpected_element(elem)),
        })?;
        Ok(())
    }

    fn read_namespace(
        &mut self,
        parser: &mut XmlParser,
        elem: &Element,
        package: Option<String>,
        c_includes: Vec<String>,
    ) -> Result<(), String> {
        let ns_name = elem.attr_required("name")?;
        let ns_id = self.add_namespace(ns_name);

        {
            let ns = self.namespace_mut(ns_id);
            ns.package_name = package;
            ns.c_includes = c_includes;
            if let Some(s) = elem.attr("shared-library") {
                ns.shared_library = s.split(',').map(String::from).collect();
            }
            if let Some(s) = elem.attr("identifier-prefixes") {
                ns.identifier_prefixes =
                    s.split(',').map(String::from).collect();
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
                _ => {
                    warn!("<{} name={:?}>", elem.name(), elem.attr("name"));
                    parser.ignore_element()
                }
            }
        })?;
        Ok(())
    }

    fn read_class(&mut self, parser: &mut XmlParser, ns_id: u16, elem: &Element) -> Result<(), String> {
        let class_name = elem.attr_required("name")?;
        let c_type = elem.attr("type")
            .or_else(|| elem.attr("type-name"))
            .ok_or_else(|| parser.fail("Missing c:type/glib:type-name attributes"))?;
        let type_struct = elem.attr("type-struct").map(|s| s.to_owned());
        let get_type = elem.attr_required("get-type")?;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut fns = Vec::new();
        let mut signals = Vec::new();
        let mut properties = Vec::new();
        let mut impls = Vec::new();
        let mut fields = Vec::new();
        let mut doc = None;
        let mut union_count = 1;

        parser.elements(|parser, elem| match elem.name() {
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            },
            "implements" => {
                self.read_type(parser, ns_id, elem).map(|r| {
                    impls.push(r.0);
                })
            }
            "signal" => {
                self.read_signal(parser, ns_id, elem).map(|s| {
                    signals.push(s)
                })
            }
            "property" => {
                self.read_property(parser, ns_id, elem).map(|p| {
                    if let Some(p) = p {
                        properties.push(p);
                    }
                })
            }
            "field" => {
                self.read_field(parser, ns_id, elem).map(|f| {
                    fields.push(f);
                })
            }
            "virtual-method" => parser.ignore_element(),
            "doc" => parser.text().map(|t| doc = Some(t)),
            "union" => {
                self.read_union(parser, ns_id, elem, Some(class_name), Some(c_type)).map(|mut u| {
                    let field_name = if let Some(field_name) = elem.attr("name") {
                        field_name.into()
                    } else {
                        format!("u{}", union_count)
                    };

                    u = Union {
                        name: format!("{}_{}", class_name, field_name),
                        c_type: Some(format!("{}_{}", c_type, field_name)),
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
                })
            }
            _ => Err(parser.unexpected_element(elem)),
        })?;

        let parent = elem.attr("parent").map(|s| self.find_or_stub_type(ns_id, s));
        let typ = Type::Class(Class {
            name: class_name.into(),
            c_type: c_type.into(),
            type_struct,
            c_class_type: None, // this will be resolved during postprocessing
            glib_get_type: get_type.into(),
            fields,
            functions: fns,
            signals,
            properties,
            parent,
            implements: impls,
            doc,
            version,
            deprecated_version,
        });
        self.add_type(ns_id, class_name, typ);
        Ok(())
    }

    fn read_record_start(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        if let Some(typ) = self.read_record(parser, ns_id, elem, None, None)? {
            let name = typ.get_name().clone();
            self.add_type(ns_id, &name, typ);
        }
        Ok(())
    }

    fn read_record(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
        parent_name_prefix: Option<&str>,
        parent_ctype_prefix: Option<&str>,
    ) -> Result<Option<Type>, String> {
        let record_name = elem.attr_required("name")?;
        let c_type = elem.attr_required("type")?;
        let get_type = elem.attr("get-type").map(|s| s.to_owned());
        let gtype_struct_for = elem.attr("is-gtype-struct-for");
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut fields = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;
        let mut union_count = 1;

        parser.elements(|parser, elem| match elem.name() {
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            }
            "union" => {
                self.read_union(parser, ns_id, elem, Some(record_name), Some(c_type)).map(|mut u| {
                    let field_name = if let Some(field_name) = elem.attr("name") {
                        field_name.into()
                    } else {
                        format!("u{}", union_count)
                    };

                    u = Union {
                        name: format!(
                                  "{}{}_{}",
                                  parent_name_prefix
                                  .map(|s| {
                                      let mut s = String::from(s);
                                      s.push('_');
                                      s
                                  })
                                  .unwrap_or_else(String::new),
                                  record_name,
                                  field_name
                                  ),
                                  c_type: Some(format!(
                                          "{}{}_{}",
                                          parent_ctype_prefix
                                          .map(|s| {
                                              let mut s = String::from(s);
                                              s.push('_');
                                              s
                                          })
                                          .unwrap_or_else(String::new),
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
                })
            }
            "field" => {
                self.read_field(parser, ns_id, elem).map(|mut f| {
                    // Workaround for wrong GValue c:type
                    if c_type == "GValue" && f.name == "data" {
                        f.c_type = Some("GValue_data".into());
                    }
                    fields.push(f);
                })
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        // Currently g-ir-scanner is too eager to mark all typedef to non-complete
        // types as disguised, so we limit this behaviour to the two most common cases.
        // https://gitlab.gnome.org/GNOME/gobject-introspection/issues/101
        let disguised = c_type == "GdkAtom" || c_type == "GIConv";

        let typ = Type::Record(Record {
            name: record_name.into(),
            c_type: c_type.into(),
            glib_get_type: get_type,
            gtype_struct_for: gtype_struct_for.map(|s| s.into()),
            fields,
            functions: fns,
            version,
            deprecated_version,
            doc,
            doc_deprecated,
            disguised,
        });

        Ok(Some(typ))
    }

    fn read_named_union(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        // Require a name here
        elem.attr_required("name")?;

        self.read_union(parser, ns_id, elem, None, None).and_then(|mut u| {
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
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
        parent_name_prefix: Option<&str>,
        parent_ctype_prefix: Option<&str>,
    ) -> Result<Union, String> {
        let union_name = elem.attr("name").unwrap_or("");
        let c_type = elem.attr("type").unwrap_or("");
        let get_type = elem.attr("get-type").map(|s| s.into());

        let mut fields = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut struct_count = 1;

        parser.elements(|parser, elem| match elem.name() {
            "field" => {
                self.read_field(parser, ns_id, elem).map(|f| {
                    fields.push(f);
                })
            }
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
                    format!("s{}", struct_count)
                };

                r = Record {
                    name: format!(
                              "{}{}_{}",
                              parent_name_prefix
                              .map(|s| {
                                  let mut s = String::from(s);
                                  s.push('_');
                                  s
                              })
                              .unwrap_or_else(String::new),
                              union_name,
                              field_name
                              ),
                              c_type: format!(
                                  "{}{}_{}",
                                  parent_ctype_prefix
                                  .map(|s| {
                                      let mut s = String::from(s);
                                      s.push('_');
                                      s
                                  })
                                  .unwrap_or_else(String::new),
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
            _ => Err(parser.unexpected_element(elem)),
        })?;

        Ok(Union {
            name: union_name.into(),
            c_type: Some(c_type.into()),
            glib_get_type: get_type,
            fields,
            functions: fns,
            doc,
        })
    }

    fn read_field(&mut self, parser: &mut XmlParser, ns_id: u16,
                  elem: &Element) -> Result<Field, String> {
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
                self.read_function(parser, ns_id, elem.name(), elem).map(|f| {
                    typ = Some((Type::function(self, f), None, None));
                })
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
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
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        self.read_function_if_not_moved(parser, ns_id, elem.name(), elem)?
            .map(|func| self.add_type(ns_id, &func.name.clone(), Type::Function(func)));

        Ok(())
    }

    fn read_interface(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        let interface_name = elem.attr_required("name")?;
        let c_type = elem.attr_required("type")?;
        let type_struct = elem.attr("type-struct").map(|s| s.to_owned());
        let get_type = elem.attr_required("get-type")?;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut fns = Vec::new();
        let mut signals = Vec::new();
        let mut properties = Vec::new();
        let mut prereqs = Vec::new();
        let mut doc = None;

        parser.elements(|parser, elem| match elem.name() {
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            }
            "prerequisite" => {
                self.read_type(parser, ns_id, elem).map(|r| {
                    prereqs.push(r.0);
                })
            }
            "signal" => {
                self.read_signal(parser, ns_id, elem).map(|s| {
                    signals.push(s)
                })
            }
            "property" => {
                self.read_property(parser, ns_id, elem).map(|p| {
                    if let Some(p) = p {
                        properties.push(p);
                    }
                })
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "virtual-method" => parser.ignore_element(),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        let typ = Type::Interface(Interface {
            name: interface_name.into(),
            c_type: c_type.into(),
            type_struct,
            c_class_type: None, // this will be resolved during postprocessing
            glib_get_type: get_type.into(),
            functions: fns,
            signals,
            properties,
            prerequisites: prereqs,
            doc,
            version,
            deprecated_version,
        });
        self.add_type(ns_id, interface_name, typ);
        Ok(())
    }

    fn read_bitfield(&mut self, parser: &mut XmlParser, ns_id: u16, elem: &Element) -> Result<(), String> {
        let bitfield_name = elem.attr_required("name")?;
        let c_type = elem.attr_required("type")?;
        let get_type = elem.attr("get-type").map(|s| s.into());
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut members = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "member" => self.read_member(parser, elem).map(|m| members.push(m)),
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
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
        });
        self.add_type(ns_id, bitfield_name, typ);
        Ok(())
    }

    fn read_enumeration(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        let enum_name = elem.attr_required("name")?;
        let c_type = elem.attr_required("type")?;
        let get_type = elem.attr("get-type").map(|s| s.into());
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let error_domain = elem.attr("error-domain").map(String::from);

        let mut members = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "member" => {
                self.read_member(parser, elem).map(|m| {
                    members.push(m)
                })
            }
            "constructor" | "function" | "method" => {
                self.read_function_to_vec(parser, ns_id, elem, &mut fns)
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
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
        });
        self.add_type(ns_id, enum_name, typ);
        Ok(())
    }

    fn read_global_function(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(), String> {
        self.read_function_if_not_moved(
            parser,
            ns_id,
            "global",
            elem,
        ).map(|func| {
            if let Some(func) = func {
                self.add_function(ns_id, func);
            }
        })
    }

    fn read_constant(
        &mut self,
        parser: &mut XmlParser,
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
                    return Err(parser.fail("Too many <type> elements"));
                }
                let (typ, c_type, array_length) = self.read_type(parser, ns_id, elem)?;
                if let Some(c_type) = c_type {
                    inner = Some((typ, c_type, array_length));
                } else {
                    return Err(parser.fail_with_position("Missing constant's c:type",
                                                         elem.position()));
                }
                Ok(())
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
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
            Err(parser.fail("Missing <type> element"))
        }
    }

    fn read_alias(&mut self, parser: &mut XmlParser, ns_id: u16,
                  elem: &Element) -> Result<(), String> {
        let alias_name = elem.attr_required("name")?;
        let c_identifier = elem.attr_required("type")?;

        let mut inner = None;
        let mut doc = None;

        parser.elements(|parser, elem| match elem.name() {
            "type" | "array" => {
                if inner.is_some() {
                    return Err(parser.fail("Too many <type> elements"));
                }
                let (typ, c_type, array_length) = self.read_type(parser, ns_id, elem)?;
                if let Some(c_type) = c_type {
                    inner = Some((typ, c_type, array_length));
                } else {
                    return Err(parser.fail("Missing alias target's c:type"));
                }
                Ok(())
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        if let Some((typ, c_type, _array_length)) = inner {
            let typ = Type::Alias(Alias {
                name: alias_name.into(),
                c_identifier: c_identifier.into(),
                typ,
                target_c_type: c_type,
                doc,
            });
            self.add_type(ns_id, alias_name, typ);
            Ok(())
        } else {
            Err(parser.fail("Missing <type> element"))
        }
    }

    fn read_member(&self, parser: &mut XmlParser, elem: &Element) -> Result<Member, String> {
        let member_name = elem.attr_required("name")?;
        let value = elem.attr_required("value")?;
        let c_identifier = elem.attr("identifier").map(|x| x.into());

        let mut doc = None;

        parser.elements(|parser, elem| match elem.name() {
            "doc" => parser.text().map(|t| doc = Some(t)),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        Ok(Member {
            name: member_name.into(),
            value: value.into(),
            doc,
            c_identifier: c_identifier.unwrap_or_else(|| member_name.into()),
        })
    }

    fn read_function(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        kind_str: &str,
        elem: &Element,
    ) -> Result<Function, String> {
        let fn_name = elem.attr_required("name")?;
        let c_identifier = elem.attr("identifier").or_else(|| elem.attr("type"));
        let kind = FunctionKind::from_str(kind_str).or_else(|why| Err(parser.fail(&why)))?;
        let is_method = kind == FunctionKind::Method;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut params = Vec::new();
        let mut ret = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "parameters" => {
                self.read_parameters(parser, ns_id, false, is_method).map(|mut ps| {
                    params.append(&mut ps)
                })
            }
            "return-value" => {
                if ret.is_some() {
                    return Err(parser.fail("Too many <return-value> elements"));
                }
                ret = Some(self.read_parameter(
                            parser,
                            ns_id,
                            elem,
                            false,
                            is_method
                            )?);
                Ok(())
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            "doc-version" => parser.ignore_element(),
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
                allow_none: true,
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
            Err(parser.fail("Missing <return-value> element"))
        }
    }

    fn read_function_to_vec(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
        fns: &mut Vec<Function>,
    ) -> Result<(), String> {
        if let Some(f) = self.read_function_if_not_moved(parser, ns_id, elem.name(), elem)? {
            fns.push(f)
        }
        Ok(())
    }

    fn read_function_if_not_moved(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        kind_str: &str,
        elem: &Element,
    ) -> Result<Option<Function>, String> {
        if elem.attr("moved-to").is_some() {
            return parser.ignore_element().map(|_| None)
        }
        self.read_function(parser, ns_id, kind_str, elem).and_then(|f| {
            if f.c_identifier.is_none() {
                return Err(parser.fail_with_position("Missing c:identifier attribute",
                                                     elem.position()));
            }
            Ok(Some(f))
        })
    }

    fn read_signal(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
    ) -> Result<Signal, String> {
        let signal_name = elem.attr_required("name")?;
        let is_action = elem.attr_bool("action", false);
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut params = Vec::new();
        let mut ret = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "parameters" => {
                self.read_parameters(parser, ns_id, true, false).map(|mut ps| {
                    params.append(&mut ps)
                })
            }
            "return-value" => {
                if ret.is_some() {
                    return Err(parser.fail("Too many <return-value> elements"));
                }
                self.read_parameter(parser, ns_id, elem, true, false).map(|p| {
                    ret = Some(p)
                })
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
            "doc-deprecated" => parser.text().map(|t| doc_deprecated = Some(t)),
            _ => Err(parser.unexpected_element(elem)),
        })?;
        if let Some(ret) = ret {
            Ok(Signal {
                name: signal_name.into(),
                parameters: params,
                ret,
                is_action,
                version,
                deprecated_version,
                doc,
                doc_deprecated,
            })
        } else {
            Err(parser.fail("Missing <return-value> element"))
        }
    }

    fn read_parameters(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        allow_no_ctype: bool,
        for_method: bool,
    ) -> Result<Vec<Parameter>, String> {
        parser.elements(|parser, elem| match elem.name() {
            "parameter" | "instance-parameter" => {
                self.read_parameter(
                    parser,
                    ns_id,
                    elem,
                    allow_no_ctype,
                    for_method)
            }
            _ => Err(parser.unexpected_element(elem)),
        })
    }

    fn read_parameter(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
        allow_no_ctype: bool,
        for_method: bool,
    ) -> Result<Parameter, String> {
        let param_name = elem.attr("name").unwrap_or("");
        let instance_parameter = elem.name() == "instance-parameter";
        let transfer = elem.attr_from_str("transfer-ownership")?.unwrap_or(Transfer::None);
        let nullable = elem.attr_bool("nullable", false);
        let allow_none = elem.attr_bool("allow-none", false);
        let scope = elem.attr_from_str("scope")?.unwrap_or(ParameterScope::None);
        let closure = elem.attr_from_str("closure")?;
        let destroy = elem.attr_from_str("destroy")?;
        let caller_allocates = elem.attr_bool("caller-allocates", false);
        let direction = if elem.name() == "return-value" {
            Ok(ParameterDirection::Return)
        } else {
            ParameterDirection::from_str(elem.attr("direction").unwrap_or("in"))
                .or_else(|why| Err(parser.fail(&why)))
        }?;

        let mut typ = None;
        let mut varargs = false;
        let mut doc = None;

        parser.elements(|parser, elem| match elem.name() {
            "type" | "array" => {
                if typ.is_some() {
                    return Err(parser.fail_with_position("Too many <type> elements",
                                                         elem.position()));
                }
                typ = Some(self.read_type(parser, ns_id, elem)?);
                if let Some((tid, None, _)) = typ {
                    if allow_no_ctype {
                        typ = Some((tid, Some(EMPTY_CTYPE.to_owned()), None));
                    } else {
                        return Err(parser.fail_with_position("Missing c:type attribute",
                                                             elem.position()));
                    }
                }
                Ok(())
            }
            "varargs" => {
                varargs = true;
                parser.ignore_element()
            }
            "doc" => parser.text().map(|t| doc = Some(t)),
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
                allow_none,
                array_length,
                is_error: false,
                doc,
                scope,
                closure,
                destroy,
            })
        } else if varargs {
            Ok(Parameter {
                name: "".into(),
                typ: self.find_type(INTERNAL_NAMESPACE, "varargs").unwrap(),
                c_type: "".into(),
                instance_parameter,
                direction: Default::default(),
                transfer: Transfer::None,
                caller_allocates: false,
                nullable: Nullable(false),
                allow_none,
                array_length: None,
                is_error: false,
                doc,
                scope,
                closure,
                destroy,
            })
        } else {
            Err(parser.fail("Missing <type> element"))
        }
    }

    fn read_property(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
    ) -> Result<Option<Property>, String> {
        let prop_name = elem.attr_required("name")?;
        let readable = elem.attr_bool("readable", true);
        let writable = elem.attr_bool("writable", false);
        let construct = elem.attr_bool("construct", false);
        let construct_only = elem.attr_bool("construct-only", false);
        let transfer = Transfer::from_str(elem.attr("transfer-ownership").unwrap_or("none"))
                .or_else(|why| Err(parser.fail(&why)))?;

        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let mut has_empty_type_tag = false;
        let mut typ = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        parser.elements(|parser, elem| match elem.name() {
            "type" | "array" => {
                if typ.is_some() {
                    return Err(parser.fail_with_position("Too many <type> elements",
                                                         elem.position()));
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
            Err(parser.fail("Missing <type> element"))
        }
    }

    fn read_type(
        &mut self,
        parser: &mut XmlParser,
        ns_id: u16,
        elem: &Element,
    ) -> Result<(TypeId, Option<String>, Option<u32>), String> {
        let type_name =
            elem.attr("name")
                .or_else(|| if elem.name() == "array" {
                    Some("array")
                } else {
                    None
                })
                .ok_or_else(|| parser.fail_with_position("Missing type name", elem.position()))?;
        let c_type = elem.attr("type").map(|s| s.into());
        let array_length = elem.attr("length").and_then(|s| s.parse().ok());

        let inner = parser.elements(|parser, elem| match elem.name() {
            "type" | "array" => self.read_type(parser, ns_id, elem).map(|r| r.0),
            _ => Err(parser.unexpected_element(elem)),
        })?;

        if inner.is_empty() || type_name == "GLib.ByteArray" {
            if type_name == "array" {
                Err(parser.fail_with_position("Missing element type", elem.position()))
            } else {
                Ok((
                    self.find_or_stub_type(ns_id, type_name),
                    c_type,
                    array_length,
                ))
            }
        } else {
            let tid = if type_name == "array" {
                Type::c_array(
                    self,
                    inner[0],
                    elem.attr("fixed-size").and_then(|n| n.parse().ok()),
                )
            } else {
                Type::container(self, type_name, inner)
                    .ok_or_else(|| parser.fail_with_position("Unknown container type", elem.position()))?
            };
            Ok((tid, c_type, array_length))
        }
    }

    fn read_version(&mut self, parser: &XmlParser, ns_id: u16,
                    elem: &Element) -> Result<Option<Version>, String> {
        self.read_version_attribute(parser, ns_id, elem, "version")
    }

    fn read_deprecated_version(&mut self, parser: &XmlParser, ns_id: u16,
                               elem: &Element) -> Result<Option<Version>, String> {
        self.read_version_attribute(parser, ns_id, elem, "deprecated-version")
    }

    fn read_version_attribute(&mut self, parser: &XmlParser, ns_id: u16, elem: &Element,
                              attr: &str) -> Result<Option<Version>, String> {
        if let Some(v) = elem.attr(attr) {
            match v.parse() {
                Ok(v) => {
                    self.register_version(ns_id, v);
                    Ok(Some(v))
                }
                Err(e) => Err(parser.fail(&format!("Invalid `{}` attribute: {}", attr, e))),
            }
        } else {
            Ok(None)
        }
    }
}

fn make_file_name(dir: &Path, name: &str) -> PathBuf {
    let mut path = dir.to_path_buf();
    let name = format!("{}.gir", name);
    path.push(name);
    path
}
