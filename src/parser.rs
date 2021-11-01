use crate::{library::*, version::Version, xmlparser::XmlParser};
use log::{trace, warn};
use quick_xml::events::BytesStart;
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

const EMPTY_CTYPE: &str = "/*EMPTY*/";

macro_rules! from_utf8 {
    ($s:expr) => {
        std::str::from_utf8(&$s).expect("failed to convert to str")
    };
}

macro_rules! attr_value {
    ($s:expr) => {
        std::str::from_utf8(&$s.value).unwrap()
    };
}

macro_rules! attr {
    ($elem:ident, $attr_name:expr) => {{
        $elem
            .attributes()
            .filter_map(|n| n.ok())
            .find(|n| n.key == $attr_name)
    }};
}

macro_rules! attr_bool {
    ($elem:ident, $attr_name:expr, $default:expr) => {{
        if let Some(attr) = $elem
            .attributes()
            .filter_map(|n| n.ok())
            .find(|n| n.key == $attr_name)
        {
            &*attr.value == b"1"
        } else {
            $default
        }
    }};
}

macro_rules! attr_or_err {
    ($elem:ident, $attr_name:expr) => {{
        let name = attr!($elem, $attr_name);
        name.ok_or_else(|| {
            format!(
                "Attribute `{}` on element <{}> is required",
                unsafe { std::str::from_utf8_unchecked($attr_name) },
                unsafe { std::str::from_utf8_unchecked($elem.name()) },
            )
        })
    }};
}

macro_rules! elements {
    ($parser:ident, $buf:ident, $var_name:ident, $callback:block) => {{
        loop {
            match $parser.next_event($buf) {
                Ok(quick_xml::events::Event::Start(_)) => {
                    let $var_name = $parser.next_element($buf)?;
                    $callback?;
                    $parser.end_element($buf)?;
                }
                _ => break,
            }
        }
    }};
}

// Basically the same as `elements` but fill a vec with the results.
macro_rules! elements_vec {
    ($parser:ident, $buf:ident, $var_name:ident, $callback:block) => {{
        let mut results = Vec::new();
        loop {
            match $parser.next_event($buf) {
                Ok(quick_xml::events::Event::Start(_)) => {
                    let $var_name = $parser.next_element($buf)?;
                    results.push($callback?);
                    $parser.end_element($buf)?;
                }
                _ => break,
            }
        }
        results
    }};
}

macro_rules! read_object_c_type {
    ($parser:ident, $elem:ident) => {{
        let attr = attr!($elem, b"type").or_else(|| attr!($elem, b"type-name"));
        attr.ok_or_else(|| {
            $parser.error(&format!(
                "Missing `c:type`/`glib:type-name` attributes on element <{}>",
                from_utf8!($elem.name()),
            ))
        })
    }};
}

pub fn is_empty_c_type(c_type: &str) -> bool {
    c_type == EMPTY_CTYPE
}

impl Library {
    pub fn read_file<P: AsRef<Path>>(
        &mut self,
        dirs: &[P],
        libs: &mut Vec<String>,
    ) -> Result<(), String> {
        let mut buf = Vec::new();

        for dir in dirs {
            buf.clear();
            let dir: &Path = dir.as_ref();
            let file_name = make_file_name(dir, &libs[libs.len() - 1]);

            let mut p = XmlParser::new(file_name)?;

            if let Err(e) = p.get_next_if_tag_is(&mut buf, b"repository") {
                return Err(e);
            }
            return self.read_repository(dirs, &mut p, &mut buf, libs);
            // match reader.read_event(&mut buf) {
            //     Ok(Event::Start(ref e)) => {
            //         match e.name() {
            //             b"repository" => self.read_repository(dirs, libs, &mut reader, &mut buf),
            //             tag => return Err(),
            //         }
            //     }
            // }
            //     Ok(Event::Text(e)) => txt.push(e.unescape_and_decode(&reader).unwrap()),
            //     Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
            //     Ok(Event::Eof) => break,
            // }
            // return parser.document(|p, _| {
            //     p.element_with_name("repository", |sub_parser, _elem| {
            //         self.read_repository(dirs, sub_parser, libs)
            //     })
            // });
        }
        Err(format!("Couldn't find `{}`...", &libs[libs.len() - 1]))
    }

    fn read_repository<'a, P: AsRef<Path>>(
        &mut self,
        dirs: &[P],
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        libs: &mut Vec<String>,
    ) -> Result<(), String> {
        let mut package = None;
        let mut includes = Vec::new();
        elements!(parser, buf, elem, {
            match elem.name() {
                b"include" => {
                    if let Some(name) = attr!(elem, b"name") {
                        let name = attr_value!(name);
                        if let Some(ver) = attr!(elem, b"version") {
                            if self.find_namespace(name).is_none() {
                                let lib = format!("{}-{}", name, attr_value!(ver));
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
                        } else {
                            includes.push(name.to_owned());
                        }
                    }
                    Ok(())
                }
                b"package" => {
                    // Take the first package element and ignore any other ones.
                    if package.is_none() {
                        let name = attr_or_err!(elem, b"name")?;
                        package = Some(attr_value!(name).to_owned());
                    }
                    Ok(())
                }
                b"namespace" => self.read_namespace(
                    parser,
                    buf,
                    &elem,
                    package.take(),
                    std::mem::take(&mut includes),
                ),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });
        Ok(())
    }

    fn read_namespace<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        elem: &BytesStart<'_>,
        package: Option<String>,
        c_includes: Vec<String>,
    ) -> Result<(), String> {
        let ns_name = attr_or_err!(elem, b"name")?;
        let ns_name = attr_value!(ns_name);
        let ns_id = self.add_namespace(ns_name);

        {
            let ns = self.namespace_mut(ns_id);
            ns.package_name = package;
            ns.c_includes = c_includes;
            if let Some(s) = attr!(elem, b"shared-library") {
                ns.shared_library = attr_value!(s).split(',').map(String::from).collect();
            }
            if let Some(s) = attr!(elem, b"identifier-prefixes") {
                ns.identifier_prefixes = attr_value!(s).split(',').map(String::from).collect();
            }
            if let Some(s) = attr!(elem, b"symbol-prefixes") {
                ns.symbol_prefixes = attr_value!(s).split(',').map(String::from).collect();
            }
        }

        trace!(
            "Reading {}-{}",
            ns_name,
            attr!(elem, b"version").map(|a| attr_value!(a)).unwrap_or("?"),
        );

        elements!(parser, buf, elem, {
            trace!(
                "<{} name={:?}>",
                from_utf8!(elem.name()),
                attr!(elem, b"name").map(|a| attr_value!(a)),
            );
            match elem.name() {
                b"class" => self.read_class(parser, buf, ns_id, &elem),
                b"record" => self.read_record_start(parser, buf, ns_id, &elem),
                b"union" => self.read_named_union(parser, buf, ns_id, &elem),
                b"interface" => self.read_interface(parser, buf, ns_id, &elem),
                b"callback" => self.read_named_callback(parser, buf, ns_id, &elem),
                b"bitfield" => self.read_bitfield(parser, buf, ns_id, &elem),
                b"enumeration" => self.read_enumeration(parser, buf, ns_id, &elem),
                b"function" => self.read_global_function(parser, buf, ns_id, &elem),
                b"constant" => self.read_constant(parser, buf, ns_id, &elem),
                b"alias" => self.read_alias(parser, buf, ns_id, &elem),
                b"function-macro" | b"docsection" => parser.ignore_element(buf),
                _ => {
                    warn!(
                        "<{} name={:?}>",
                        from_utf8!(elem.name()),
                        attr!(elem, b"name").map(|a| attr_value!(a)),
                    );
                    parser.ignore_element(buf)
                }
            }
        });
        Ok(())
    }

    fn read_class<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(), String> {
        let class_name = attr_or_err!(elem, b"name")?;
        let class_name = attr_value!(class_name);
        let c_type = read_object_c_type!(parser, elem)?;
        let c_type = attr_value!(c_type);
        let symbol_prefix = attr_or_err!(elem, b"symbol-prefix").map(|a| attr_value!(a).to_owned())?;
        let type_struct = attr!(elem, b"type-struct").map(|a| attr_value!(a).to_owned());
        let get_type = attr_or_err!(elem, b"get-type")?;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let is_abstract = attr!(elem, b"abstract").map(|x| &*x.value == b"1").unwrap_or(false);

        let mut fns = Vec::new();
        let mut signals = Vec::new();
        let mut properties = Vec::new();
        let mut impls = Vec::new();
        let mut fields = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;
        let mut union_count = 1;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"constructor" | b"function" | b"method" => {
                    self.read_function_to_vec(parser, buf, ns_id, &elem, &mut fns)
                }
                b"implements" => self.read_type(parser, buf, ns_id, &elem).map(|r| {
                    impls.push(r.0);
                }),
                b"signal" => self
                    .read_signal(parser, buf, ns_id, &elem)
                    .map(|s| signals.push(s)),
                b"property" => self.read_property(parser, buf, ns_id, &elem).map(|p| {
                    if let Some(p) = p {
                        properties.push(p);
                    }
                }),
                b"field" => self.read_field(parser, buf, ns_id, &elem).map(|f| {
                    fields.push(f);
                }),
                b"virtual-method" => parser.ignore_element(buf),
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"doc-deprecated" => parser
                    .text(buf, b"doc-deprecated")
                    .map(|t| doc_deprecated = Some(t)),
                b"source-position" => parser.ignore_element(buf),
                b"union" => self
                    .read_union(parser, buf, ns_id, &elem, Some(class_name), Some(c_type))
                    .map(|mut u| {
                        let field_name = if let Some(field_name) = attr!(elem, b"name") {
                            attr_value!(field_name).into()
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
                    }),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        let parent = attr!(elem, b"parent").map(|s| self.find_or_stub_type(ns_id, attr_value!(s)));
        let typ = Type::Class(Class {
            name: class_name.into(),
            c_type: c_type.into(),
            type_struct,
            c_class_type: None, // this will be resolved during postprocessing
            glib_get_type: attr_value!(get_type).into(),
            fields,
            functions: fns,
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
        });
        self.add_type(ns_id, class_name, typ);
        Ok(())
    }

    fn read_record_start<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(), String> {
        if let Some(typ) = self.read_record(parser, buf, ns_id, elem, None, None)? {
            let name = typ.get_name();
            self.add_type(ns_id, &name, typ);
        }
        Ok(())
    }

    fn read_record<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
        parent_name_prefix: Option<&str>,
        parent_ctype_prefix: Option<&str>,
    ) -> Result<Option<Type>, String> {
        let record_name = attr_or_err!(elem, b"name")?;
        let record_name = attr_value!(record_name);
        let c_type = attr_or_err!(elem, b"type")?;
        let c_type = attr_value!(c_type);
        let symbol_prefix = attr!(elem, b"symbol-prefix").map(|a| attr_value!(a).to_owned());
        let get_type = attr!(elem, b"get-type").map(|a| attr_value!(a).to_owned());
        let gtype_struct_for = attr!(elem, b"is-gtype-struct-for");
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let disguised = attr_bool!(elem, b"disguised", false);

        let mut fields = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;
        let mut union_count = 1;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"constructor" | b"function" | b"method" => {
                    self.read_function_to_vec(parser, buf, ns_id, &elem, &mut fns)
                }
                b"union" => self
                    .read_union(parser, buf, ns_id, &elem, Some(record_name), Some(c_type))
                    .map(|mut u| {
                        let field_name = if let Some(field_name) = attr!(elem, b"name") {
                            attr_value!(field_name).into()
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
                    }),
                b"field" => {
                    self.read_field(parser, buf, ns_id, &elem).map(|mut f| {
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
                        } else {
                            // Workaround for wrong GValue c:type
                            if c_type == "GValue" && f.name == "data" {
                                f.c_type = Some("GValue_data".into());
                            }
                            fields.push(f);
                        }
                    })
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"doc-deprecated" => parser
                    .text(buf, b"doc-deprecated")
                    .map(|t| doc_deprecated = Some(t)),
                b"source-position" => parser.ignore_element(buf),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        let typ = Type::Record(Record {
            name: record_name.into(),
            c_type: c_type.into(),
            glib_get_type: get_type,
            gtype_struct_for: gtype_struct_for.map(|s| attr_value!(s).into()),
            fields,
            functions: fns,
            version,
            deprecated_version,
            doc,
            doc_deprecated,
            disguised,
            symbol_prefix,
        });

        Ok(Some(typ))
    }

    fn read_named_union<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(), String> {
        // Require a name here
        attr_or_err!(elem, b"name")?;

        self.read_union(parser, buf, ns_id, elem, None, None)
            .and_then(|mut u| {
                assert_ne!(u.name, "");
                // Workaround for missing c:type
                if u.name == "_Value__data__union" {
                    u.c_type = Some("GValue_data".into());
                } else if u.c_type.is_none() {
                    return Err(parser.error("Missing union c:type"));
                }
                let union_name = u.name.clone();
                self.add_type(ns_id, &union_name, Type::Union(u));
                Ok(())
            })
    }

    fn read_union<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
        parent_name_prefix: Option<&str>,
        parent_ctype_prefix: Option<&str>,
    ) -> Result<Union, String> {
        let union_name = attr!(elem, b"name");
        let union_name = union_name.map(|a| attr_value!(a)).unwrap_or("");
        let c_type = read_object_c_type!(parser, elem);
        let c_type = c_type.map(|a| attr_value!(a)).unwrap_or("");
        let get_type = attr!(elem, b"get-type").map(|s| attr_value!(s).into());
        let symbol_prefix = attr!(elem, b"symbol-prefix").map(|a| attr_value!(a).to_owned());

        let mut fields = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut struct_count = 1;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"source-position" => parser.ignore_element(buf),
                b"field" => self.read_field(parser, buf, ns_id, &elem).map(|f| {
                    fields.push(f);
                }),
                b"constructor" | b"function" | b"method" => {
                    self.read_function_to_vec(parser, buf, ns_id, &elem, &mut fns)
                }
                b"record" => {
                    if let Some(Type::Record(mut r)) = self.read_record(
                        parser,
                        buf,
                        ns_id,
                        &elem,
                        parent_name_prefix,
                        parent_ctype_prefix,
                    )? {
                        let field_name = if let Some(field_name) = attr!(elem, b"name") {
                            attr_value!(field_name).into()
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
                    }
                    Ok(())
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

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

    fn read_field<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<Field, String> {
        let field_name = attr_or_err!(elem, b"name")?;
        let private = attr_bool!(elem, b"private", false);
        let bits = attr!(elem, b"bits").and_then(|s| attr_value!(s).parse().ok());

        let mut typ = None;
        let mut doc = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"type" | b"array" => {
                    if typ.is_some() {
                        return Err(parser.error("Too many <type> elements"));
                    }
                    self.read_type(parser, buf, ns_id, &elem).map(|t| {
                        typ = Some(t);
                    })
                }
                b"callback" => {
                    if typ.is_some() {
                        return Err(parser.error("Too many <type> elements"));
                    }
                    self.read_function(parser, buf, ns_id, elem.name(), &elem)
                        .map(|f| {
                            typ = Some((Type::function(self, f), None, None));
                        })
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        if let Some((tid, c_type, array_length)) = typ {
            Ok(Field {
                name: attr_value!(field_name).into(),
                typ: tid,
                c_type,
                private,
                bits,
                array_length,
                doc,
            })
        } else {
            Err(parser.error("Missing <type> element"))
        }
    }

    fn read_named_callback<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(), String> {
        self.read_function_if_not_moved(parser, buf, ns_id, elem.name(), elem)?
            .map(|func| {
                let name = func.name.clone();
                self.add_type(ns_id, &name, Type::Function(func))
            });

        Ok(())
    }

    fn read_interface<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(), String> {
        let interface_name = attr_or_err!(elem, b"name")?;
        let interface_name = attr_value!(interface_name);
        let c_type = read_object_c_type!(parser, elem)?;
        let symbol_prefix = attr_or_err!(elem, b"symbol-prefix").map(|a| attr_value!(a).to_owned())?;
        let type_struct = attr!(elem, b"type-struct").map(|a| attr_value!(a).to_owned());
        let get_type = attr_or_err!(elem, b"get-type")?;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut fns = Vec::new();
        let mut signals = Vec::new();
        let mut properties = Vec::new();
        let mut prereqs = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"constructor" | b"function" | b"method" => {
                    self.read_function_to_vec(parser, buf, ns_id, &elem, &mut fns)
                }
                b"prerequisite" => self.read_type(parser, buf, ns_id, &elem).map(|r| {
                    prereqs.push(r.0);
                }),
                b"signal" => self
                    .read_signal(parser, buf, ns_id, &elem)
                    .map(|s| signals.push(s)),
                b"property" => self.read_property(parser, buf, ns_id, &elem).map(|p| {
                    if let Some(p) = p {
                        properties.push(p);
                    }
                }),
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"doc-deprecated" => parser
                    .text(buf, b"doc-deprecated")
                    .map(|t| doc_deprecated = Some(t)),
                b"virtual-method" => parser.ignore_element(buf),
                b"source-position" => parser.ignore_element(buf),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        let typ = Type::Interface(Interface {
            name: interface_name.into(),
            c_type: attr_value!(c_type).into(),
            type_struct,
            c_class_type: None, // this will be resolved during postprocessing
            glib_get_type: attr_value!(get_type).into(),
            functions: fns,
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

    fn read_bitfield<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(), String> {
        let bitfield_name = attr_or_err!(elem, b"name")?;
        let bitfield_name = attr_value!(bitfield_name);
        let c_type = read_object_c_type!(parser, elem)?;
        let symbol_prefix = attr_or_err!(elem, b"symbol-prefix")
            .map(|a| attr_value!(a).to_owned())
            .ok();
        let get_type = attr!(elem, b"get-type").map(|s| attr_value!(s).into());
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut members = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"member" => self
                    .read_member(parser, buf, ns_id, &elem)
                    .map(|m| members.push(m)),
                b"constructor" | b"function" | b"method" => {
                    self.read_function_to_vec(parser, buf, ns_id, &elem, &mut fns)
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"doc-deprecated" => parser
                    .text(buf, b"doc-deprecated")
                    .map(|t| doc_deprecated = Some(t)),
                b"source-position" => parser.ignore_element(buf),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        let typ = Type::Bitfield(Bitfield {
            name: bitfield_name.into(),
            c_type: attr_value!(c_type).into(),
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

    fn read_enumeration<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(), String> {
        let enum_name = attr_or_err!(elem, b"name")?;
        let enum_name = attr_value!(enum_name);
        let c_type = read_object_c_type!(parser, elem)?;
        let symbol_prefix = attr!(elem, b"symbol-prefix").map(|a| attr_value!(a).to_owned());
        let get_type = attr!(elem, b"get-type").map(|s| attr_value!(s).into());
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let error_domain =
            attr!(elem, b"error-domain").map(|s| ErrorDomain::Quark(String::from(attr_value!(s))));

        let mut members = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"member" => self
                    .read_member(parser, buf, ns_id, &elem)
                    .map(|m| members.push(m)),
                b"constructor" | b"function" | b"method" => {
                    self.read_function_to_vec(parser, buf, ns_id, &elem, &mut fns)
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"doc-deprecated" => parser
                    .text(buf, b"doc-deprecated")
                    .map(|t| doc_deprecated = Some(t)),
                b"source-position" => parser.ignore_element(buf),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        let typ = Type::Enumeration(Enumeration {
            name: enum_name.into(),
            c_type: attr_value!(c_type).into(),
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

    fn read_global_function<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(), String> {
        self.read_function_if_not_moved(parser, buf, ns_id, b"global", elem)
            .map(|func| {
                if let Some(func) = func {
                    self.add_function(ns_id, func);
                }
            })
    }

    fn read_constant<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(), String> {
        let const_name = attr_or_err!(elem, b"name")?;
        let c_identifier = attr_or_err!(elem, b"type")?;
        let value = attr_or_err!(elem, b"value")?;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut inner = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"type" | b"array" => {
                    if inner.is_some() {
                        return Err(parser.error_with_position(
                            "Too many <type> inner elements in <constant> element",
                        ));
                    }
                    let (typ, c_type, array_length) = self.read_type(parser, buf, ns_id, &elem)?;
                    if let Some(c_type) = c_type {
                        inner = Some((typ, c_type, array_length));
                    } else {
                        return Err(
                            parser.error_with_position("Missing <constant> element's c:type")
                        );
                    }
                    Ok(())
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"doc-deprecated" => parser
                    .text(buf, b"doc-deprecated")
                    .map(|t| doc_deprecated = Some(t)),
                b"source-position" => parser.ignore_element(buf),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        if let Some((typ, c_type, _array_length)) = inner {
            self.add_constant(
                ns_id,
                Constant {
                    name: attr_value!(const_name).into(),
                    c_identifier: attr_value!(c_identifier).into(),
                    typ,
                    c_type,
                    value: attr_value!(value).into(),
                    version,
                    deprecated_version,
                    doc,
                    doc_deprecated,
                },
            );
            Ok(())
        } else {
            Err(parser.error_with_position("Missing <type> element inside <constant> element"))
        }
    }

    fn read_alias<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(), String> {
        let alias_name = attr_or_err!(elem, b"name")?;
        let alias_name = attr_value!(alias_name);
        let c_identifier = attr_or_err!(elem, b"type")?;

        let mut inner = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"source-position" => parser.ignore_element(buf),
                b"type" | b"array" => {
                    if inner.is_some() {
                        return Err(parser.error_with_position(
                            "Too many <type> inner elements in <alias> element",
                        ));
                    }
                    let (typ, c_type, array_length) = self.read_type(parser, buf, ns_id, &elem)?;
                    if let Some(c_type) = c_type {
                        inner = Some((typ, c_type, array_length));
                    } else {
                        return Err(parser.error("Missing <alias> target's c:type"));
                    }
                    Ok(())
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"doc-deprecated" => parser
                    .text(buf, b"doc-deprecated")
                    .map(|t| doc_deprecated = Some(t)),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        if let Some((typ, c_type, _array_length)) = inner {
            let typ = Type::Alias(Alias {
                name: alias_name.into(),
                c_identifier: attr_value!(c_identifier).into(),
                typ,
                target_c_type: c_type,
                doc,
                doc_deprecated,
            });
            self.add_type(ns_id, alias_name, typ);
            Ok(())
        } else {
            Err(parser.error_with_position("Missing <type> element inside <alias> element"))
        }
    }

    fn read_member<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<Member, String> {
        let member_name = attr_or_err!(elem, b"name")?;
        let member_name = attr_value!(member_name);
        let value = attr_or_err!(elem, b"value")?;
        let value = attr_value!(value);
        let c_identifier = attr!(elem, b"identifier").map(|x| attr_value!(x).into());
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut doc = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        Ok(Member {
            name: member_name.into(),
            value: value.into(),
            doc,
            c_identifier: c_identifier.unwrap_or_else(|| member_name.into()),
            status: crate::config::gobjects::GStatus::Generate,
            version,
            deprecated_version,
        })
    }

    fn read_function<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        kind: &[u8],
        elem: &BytesStart<'_>,
    ) -> Result<Function, String> {
        let fn_name = attr_or_err!(elem, b"name")?;
        let c_identifier = attr!(elem, b"identifier").or_else(|| attr!(elem, b"type"));
        let kind_str = from_utf8!(kind);
        let kind = FunctionKind::from_str(kind_str).map_err(|why| parser.error(&why))?;
        let is_method = kind == FunctionKind::Method;
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut params = Vec::new();
        let mut ret = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"parameters" => self
                    .read_parameters(parser, buf, ns_id, false, is_method)
                    .map(|mut ps| params.append(&mut ps)),
                b"return-value" => {
                    if ret.is_some() {
                        return Err(parser.error_with_position(
                            "Too many <return-value> elements inside <function> element",
                        ));
                    }
                    ret = Some(self.read_parameter(parser, buf, ns_id, &elem, false, is_method)?);
                    Ok(())
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"doc-deprecated" => parser
                    .text(buf, b"doc-deprecated")
                    .map(|t| doc_deprecated = Some(t)),
                b"doc-version" => parser.ignore_element(buf),
                b"source-position" => parser.ignore_element(buf),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        let throws = attr_bool!(elem, b"throws", false);
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
                name: attr_value!(fn_name).into(),
                c_identifier: c_identifier.map(|s| attr_value!(s).into()),
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
            Err(parser.error_with_position("Missing <return-value> element in <function> element"))
        }
    }

    fn read_function_to_vec<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
        fns: &mut Vec<Function>,
    ) -> Result<(), String> {
        if let Some(f) = self.read_function_if_not_moved(parser, buf, ns_id, elem.name(), elem)? {
            fns.push(f)
        }
        Ok(())
    }

    fn read_function_if_not_moved<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        kind_str: &[u8],
        elem: &BytesStart<'_>,
    ) -> Result<Option<Function>, String> {
        if attr!(elem, b"moved-to").is_some() {
            return parser.ignore_element(buf).map(|_| None);
        }
        self.read_function(parser, buf, ns_id, kind_str, elem)
            .and_then(|f| {
                if f.c_identifier.is_none() {
                    return Err(parser.error_with_position(&format!(
                        "Missing c:identifier attribute in <{}> element",
                        from_utf8!(elem.name()),
                    )));
                }
                Ok(Some(f))
            })
    }

    fn read_signal<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<Signal, String> {
        let signal_name = attr_or_err!(elem, b"name")?;
        let is_action = attr_bool!(elem, b"action", false);
        let is_detailed = attr_bool!(elem, b"detailed", false);
        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;

        let mut params = Vec::new();
        let mut ret = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"parameters" => self
                    .read_parameters(parser, buf, ns_id, true, false)
                    .map(|mut ps| params.append(&mut ps)),
                b"return-value" => {
                    if ret.is_some() {
                        return Err(parser.error_with_position(
                            "Too many <return-value> elements in <signal> element",
                        ));
                    }
                    self.read_parameter(parser, buf, ns_id, &elem, true, false)
                        .map(|p| ret = Some(p))
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"doc-deprecated" => parser
                    .text(buf, b"doc-deprecated")
                    .map(|t| doc_deprecated = Some(t)),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });
        if let Some(ret) = ret {
            Ok(Signal {
                name: attr_value!(signal_name).into(),
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
            Err(parser.error_with_position("Missing <return-value> element in <signal> element"))
        }
    }

    fn read_parameters<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        allow_no_ctype: bool,
        for_method: bool,
    ) -> Result<Vec<Parameter>, String> {
        Ok(elements_vec!(parser, buf, elem, {
            match elem.name() {
                b"parameter" | b"instance-parameter" => {
                    self.read_parameter(parser, buf, ns_id, &elem, allow_no_ctype, for_method)
                }
                _ => return Err(parser.unexpected_element(&elem)),
            }
        }))
    }

    fn read_parameter<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
        allow_no_ctype: bool,
        for_method: bool,
    ) -> Result<Parameter, String> {
        let param_name = attr!(elem, b"name");
        let param_name = param_name.map(|a| attr_value!(a)).unwrap_or("");
        let instance_parameter = elem.name() == b"instance-parameter";
        let transfer = parser
            .attr_from_str(elem, b"transfer-ownership")?
            .unwrap_or(Transfer::None);
        let nullable = attr_bool!(elem, b"nullable", false);
        let allow_none = attr_bool!(elem, b"allow-none", false);
        let scope = parser
            .attr_from_str(elem, b"scope")?
            .unwrap_or(ParameterScope::None);
        let closure = parser.attr_from_str(elem, b"closure")?;
        let destroy = parser.attr_from_str(elem, b"destroy")?;
        let caller_allocates = attr_bool!(elem, b"caller-allocates", false);
        let direction = if elem.name() == b"return-value" {
            Ok(ParameterDirection::Return)
        } else {
            let direction = attr!(elem, b"direction");
            ParameterDirection::from_str(direction.map(|a| attr_value!(a)).unwrap_or("in"))
                .map_err(|why| parser.error_with_position(&why))
        }?;

        let mut typ = None;
        let mut varargs = false;
        let mut doc = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"type" | b"array" => {
                    if typ.is_some() {
                        return Err(parser.error_with_position(&format!(
                            "Too many <type> elements in <{}> element",
                            from_utf8!(elem.name()),
                        )));
                    }
                    typ = Some(self.read_type(parser, buf, ns_id, &elem)?);
                    if let Some((tid, None, _)) = typ {
                        if allow_no_ctype {
                            typ = Some((tid, Some(EMPTY_CTYPE.to_owned()), None));
                        } else {
                            return Err(parser.error_with_position(&format!(
                                "Missing c:type attribute in <{}> element",
                                from_utf8!(elem.name()),
                            )));
                        }
                    }
                    Ok(())
                }
                b"varargs" => {
                    varargs = true;
                    parser.ignore_element(buf)
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

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
            Err(parser.error_with_position(&format!(
                "Missing <type> element in <{}> element",
                from_utf8!(elem.name()),
            )))
        }
    }

    fn read_property<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<Option<Property>, String> {
        let prop_name = attr_or_err!(elem, b"name")?;
        let readable = attr_bool!(elem, b"readable", true);
        let writable = attr_bool!(elem, b"writable", false);
        let construct = attr_bool!(elem, b"construct", false);
        let construct_only = attr_bool!(elem, b"construct-only", false);
        let transfer = attr!(elem, b"transfer-ownership");
        let transfer = Transfer::from_str(transfer.map(|a| attr_value!(a)).unwrap_or("none"))
            .map_err(|why| parser.error_with_position(&why))?;

        let version = self.read_version(parser, ns_id, elem)?;
        let deprecated_version = self.read_deprecated_version(parser, ns_id, elem)?;
        let mut has_empty_type_tag = false;
        let mut typ = None;
        let mut doc = None;
        let mut doc_deprecated = None;

        elements!(parser, buf, elem, {
            match elem.name() {
                b"type" | b"array" => {
                    if typ.is_some() {
                        return Err(parser.error_with_position(
                            "Too many <type> elements in <property> element",
                        ));
                    }
                    if elem.attributes().count() == 0 && elem.name() == b"type" {
                        // defend from <type/>
                        has_empty_type_tag = true;
                        parser.ignore_element(buf)
                    } else {
                        typ = Some(self.read_type(parser, buf, ns_id, &elem)?);
                        if let Some((tid, None, _)) = typ {
                            typ = Some((tid, Some(EMPTY_CTYPE.to_owned()), None));
                        }
                        Ok(())
                    }
                }
                b"doc" => parser.text(buf, b"doc").map(|t| doc = Some(t)),
                b"doc-deprecated" => parser
                    .text(buf, b"doc-deprecated")
                    .map(|t| doc_deprecated = Some(t)),
                b"attribute" => parser.ignore_element(buf),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        if has_empty_type_tag {
            return Ok(None);
        }

        if let Some((tid, c_type, _array_length)) = typ {
            Ok(Some(Property {
                name: attr_value!(prop_name).into(),
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
            Err(parser.error_with_position("Missing <type> element in <property> element"))
        }
    }

    fn read_type<'a>(
        &mut self,
        parser: &mut XmlParser,
        buf: &'a mut Vec<u8>,

        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<(TypeId, Option<String>, Option<u32>), String> {
        let type_name = attr!(elem, b"name");
        let type_name = type_name.map(|a| attr_value!(a))
            .or_else(|| {
                if elem.name() == b"array" {
                    Some("array")
                } else {
                    None
                }
            })
            .ok_or_else(|| {
                parser.error_with_position("<type> element is missing a name attribute")
            })?;
        let c_type = attr!(elem, b"type").map(|s| attr_value!(s).into());
        let array_length = attr!(elem, b"length").and_then(|s| attr_value!(s).parse().ok());

        let inner = elements_vec!(parser, buf, elem, {
            match elem.name() {
                b"type" | b"array" => self.read_type(parser, buf, ns_id, &elem),
                _ => return Err(parser.unexpected_element(&elem)),
            }
        });

        if inner.is_empty() || type_name == "GLib.ByteArray" {
            if type_name == "array" {
                Err(parser.error_with_position("<type> element is missing an inner element type"))
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
                    attr!(elem, b"fixed-size").and_then(|n| attr_value!(n).parse().ok()),
                    inner_type.1.clone(),
                )
            } else {
                let inner = inner.iter().map(|r| r.0).collect();
                Type::container(self, type_name, inner)
                    .ok_or_else(|| parser.error_with_position("Unknown container type"))?
            };
            Ok((tid, c_type, array_length))
        }
    }

    fn read_version(
        &mut self,
        parser: &XmlParser,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<Option<Version>, String> {
        self.read_version_attribute(parser, ns_id, elem, b"version")
    }

    fn read_deprecated_version(
        &mut self,
        parser: &XmlParser,
        ns_id: u16,
        elem: &BytesStart<'_>,
    ) -> Result<Option<Version>, String> {
        self.read_version_attribute(parser, ns_id, elem, b"deprecated-version")
    }

    fn read_version_attribute(
        &mut self,
        parser: &XmlParser,
        ns_id: u16,
        elem: &BytesStart<'_>,
        attr: &[u8],
    ) -> Result<Option<Version>, String> {
        if let Some(v) = attr!(elem, attr) {
            match attr_value!(v).parse() {
                Ok(v) => {
                    self.register_version(ns_id, v);
                    Ok(Some(v))
                }
                Err(e) => {
                    Err(parser.error(&format!("Invalid `{}` attribute: {}", from_utf8!(attr), e)))
                }
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
