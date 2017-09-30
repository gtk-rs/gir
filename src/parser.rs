use std::io::BufReader;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use xml::attribute::OwnedAttribute;
use xml::common::Position;
use xml::name::OwnedName;
use xml::reader::EventReader;
use xml::reader::XmlEvent::{StartElement, EndElement, EndDocument, Characters};

use config::error::*;
use library::*;
use version::Version;

type Reader = EventReader<BufReader<File>>;
type Attributes = [OwnedAttribute];

const EMPTY_CTYPE: &'static str = "/*EMPTY*/";

pub fn is_empty_c_type(c_type: &str) -> bool {
    c_type == EMPTY_CTYPE
}

macro_rules! mk_error {
    ($msg:expr, $obj:expr) => (
        ErrorKind::GirXml(format!("{} {}:{} {}", $obj.position(), file!(), line!(), $msg))
    )
}

macro_rules! xml_next {
    ($event:expr, $pos:expr) => (
        if let EndDocument = $event {
            bail!(mk_error!("Unexpected end of document", $pos))
        }
    )
}

impl Library {
    pub fn read_file(&mut self, dir: &Path, lib: &str) -> Result<()> {
        let file_name = make_file_name(dir, lib);
        let display_name = file_name.display();
        let file = try!(
            File::open(&file_name).chain_err(|| { format!("Can't read file {}", display_name) })
        );
        let mut parser = EventReader::new(BufReader::new(file));
        loop {
            let event = parser.next();
            try!(
                match event {
                    Ok(StartElement {
                        name: OwnedName {
                            ref local_name,
                            namespace: Some(ref namespace),
                            ..
                        },
                        ..
                    }) if local_name == "repository" &&
                          namespace == "http://www.gtk.org/introspection/core/1.0" => {
                        match self.read_repository(dir, &mut parser) {
                            // To prevent repeat message in "caused by:" for each file
                            e @ Err(Error(ErrorKind::Msg(_), _)) => return e,
                            Err(e) => Err(e),
                            Ok(_) => Ok(()),
                        }
                    }
                    Ok(EndDocument) => break,
                    Err(e) => Err(e.into()),
                    _ => continue,
                }.chain_err(|| format!("Error in file {}", display_name))
            );
        }
        Ok(())
    }

    fn read_repository(&mut self, dir: &Path, parser: &mut Reader) -> Result<()> {
        let mut package = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "include" => {
                            if let (Some(lib), Some(ver)) =
                                (attributes.by_name("name"), attributes.by_name("version"))
                            {
                                if self.find_namespace(lib).is_none() {
                                    let lib = format!("{}-{}", lib, ver);
                                    try!(self.read_file(dir, &lib));
                                }
                            }
                            try!(ignore_element(parser));
                        }
                        "package" => {
                            // take the first package element and ignore any other ones
                            if package.is_none() {
                                package = attributes.by_name("name").map(|s| s.to_owned());
                                if package.is_none() {
                                    bail!(mk_error!("Missing package name", parser));
                                }
                            }
                            try!(ignore_element(parser));
                        }
                        "namespace" => {
                            try!(self.read_namespace(parser, &attributes, package.take()));
                        }
                        _ => try!(ignore_element(parser)),
                    }
                }
                EndElement { .. } => return Ok(()),
                _ => xml_next!(event, parser),
            }
        }
    }

    fn read_namespace(
        &mut self,
        parser: &mut Reader,
        attrs: &Attributes,
        package: Option<String>,
    ) -> Result<()> {
        let ns_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing namespace name", parser))
        );
        let ns_id = self.add_namespace(ns_name);
        self.namespace_mut(ns_id).package_name = package;
        if let Some(s) = attrs.by_name("shared-library") {
            self.namespace_mut(ns_id).shared_library = s.split(',').map(String::from).collect();
        }
        if let Some(s) = attrs.by_name("identifier-prefixes") {
            self.namespace_mut(ns_id).identifier_prefixes =
                s.split(',').map(String::from).collect();
        }
        if let Some(s) = attrs.by_name("symbol-prefixes") {
            self.namespace_mut(ns_id).symbol_prefixes = s.split(',').map(String::from).collect();
        }
        trace!("Reading {}-{}", ns_name, attrs.by_name("version").unwrap());
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    trace!(
                        "<{} name={:?}>",
                        name.local_name,
                        attributes.by_name("name")
                    );
                    match name.local_name.as_ref() {
                        "class" => {
                            trace!("class {}", attributes.by_name("name").unwrap());
                            try!(self.read_class(parser, ns_id, &attributes));
                        }
                        "record" => {
                            try!(self.read_record_start(parser, ns_id, &attributes));
                        }
                        "union" => {
                            try!(self.read_named_union(parser, ns_id, &attributes));
                        }
                        "interface" => {
                            try!(self.read_interface(parser, ns_id, &attributes));
                        }
                        "callback" => {
                            try!(self.read_named_callback(parser, ns_id, &attributes));
                        }
                        "bitfield" => {
                            try!(self.read_bitfield(parser, ns_id, &attributes));
                        }
                        "enumeration" => {
                            try!(self.read_enumeration(parser, ns_id, &attributes));
                        }
                        "function" => {
                            try!(self.read_global_function(parser, ns_id, &attributes));
                        }
                        "constant" => {
                            try!(self.read_constant(parser, ns_id, &attributes));
                        }
                        "alias" => {
                            try!(self.read_alias(parser, ns_id, &attributes));
                        }
                        _ => {
                            warn!(
                                "<{} name={:?}>",
                                name.local_name,
                                attributes.by_name("name")
                            );
                            try!(ignore_element(parser));
                        }
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        Ok(())
    }

    fn read_class(&mut self, parser: &mut Reader, ns_id: u16, attrs: &Attributes) -> Result<()> {
        let class_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing class name", parser))
        );
        let c_type = try!(
            attrs
                .by_name("type")
                .or_else(|| attrs.by_name("type-name"))
                .ok_or_else(|| {
                    mk_error!("Missing c:type/glib:type-name attributes", parser)
                })
        );
        let get_type = try!(
            attrs
                .by_name("get-type")
                .ok_or_else(|| mk_error!("Missing get-type attribute", parser))
        );
        let version = try!(self.parse_version(parser, ns_id, attrs.by_name("version")));
        let deprecated_version = try!(self.parse_version(
            parser,
            ns_id,
            attrs.by_name("deprecated-version"),
        ));
        let mut fns = Vec::new();
        let mut signals = Vec::new();
        let mut properties = Vec::new();
        let mut impls = Vec::new();
        let mut fields = Vec::new();
        let mut doc = None;
        let mut union_count = 1;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            try!(self.read_function_to_vec(
                                parser,
                                ns_id,
                                kind,
                                &attributes,
                                &mut fns,
                            ))
                        }
                        "implements" => {
                            impls.push(try!(self.read_type(parser, ns_id, &name, &attributes)).0)
                        }
                        "signal" => {
                            try!(self.read_signal_to_vec(
                                parser,
                                ns_id,
                                &attributes,
                                &mut signals,
                            ))
                        }
                        "property" => {
                            try!(self.read_property_to_vec(
                                parser,
                                ns_id,
                                &attributes,
                                &mut properties,
                            ))
                        }
                        "field" => fields.push(try!(self.read_field(parser, ns_id, &attributes))),
                        "virtual-method" => try!(ignore_element(parser)),
                        "doc" => doc = try!(read_text(parser)),
                        "union" => {
                            if let Type::Union(mut u) = try!(self.read_union(
                                parser,
                                ns_id,
                                &attributes,
                                Some(class_name),
                                Some(c_type)
                            )) {
                                let field_name =
                                    if let Some(field_name) = attributes.by_name("name") {
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

                                let type_id = {
                                    #[cfg(not(feature = "use_unions"))]
                                    {
                                        Type::union(self, u, INTERNAL_NAMESPACE)
                                    }
                                    #[cfg(feature = "use_unions")]
                                    {
                                        Type::union(self, u, ns_id)
                                    }
                                };

                                fields.push(Field {
                                    name: field_name,
                                    typ: type_id,
                                    doc: u_doc,
                                    c_type: ctype,
                                    ..Field::default()
                                });
                                union_count += 1;
                            }
                        }
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }

        let parent = attrs
            .by_name("parent")
            .map(|s| self.find_or_stub_type(ns_id, s));
        let typ = Type::Class(Class {
            name: class_name.into(),
            c_type: c_type.into(),
            glib_get_type: get_type.into(),
            fields: fields,
            functions: fns,
            signals: signals,
            properties: properties,
            parent: parent,
            implements: impls,
            doc: doc,
            version: version,
            deprecated_version: deprecated_version,
        });
        self.add_type(ns_id, class_name, typ);
        Ok(())
    }

    fn read_record_start(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
    ) -> Result<()> {

        if let Some(typ) = try!(self.read_record(parser, ns_id, attrs, None, None)) {
            let name = typ.get_name().clone();
            self.add_type(ns_id, &name, typ);
        }
        Ok(())
    }

    fn read_record(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
        parent_name_prefix: Option<&str>,
        parent_ctype_prefix: Option<&str>,
    ) -> Result<Option<Type>> {
        let mut record_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing record name", parser))
        );
        let mut c_type = try!(
            attrs
                .by_name("type")
                .ok_or_else(|| mk_error!("Missing c:type attribute", parser))
        );
        let get_type = match attrs.by_name("get-type") {
            Some(s) => Some(s.to_string()),
            None => None,
        };
        let version = try!(self.parse_version(parser, ns_id, attrs.by_name("version")));
        let deprecated_version = try!(self.parse_version(
            parser,
            ns_id,
            attrs.by_name("deprecated-version"),
        ));
        let mut fields = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;
        let mut union_count = 1;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            try!(self.read_function_to_vec(
                                parser,
                                ns_id,
                                kind,
                                &attributes,
                                &mut fns,
                            ))
                        }
                        "union" => {
                            if let Type::Union(mut u) = try!(self.read_union(
                                parser,
                                ns_id,
                                &attributes,
                                Some(record_name),
                                Some(c_type)
                            )) {
                                let field_name =
                                    if let Some(field_name) = attributes.by_name("name") {
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

                                let type_id = {
                                    #[cfg(not(feature = "use_unions"))]
                                    {
                                        Type::union(self, u, INTERNAL_NAMESPACE)
                                    }
                                    #[cfg(feature = "use_unions")]
                                    {
                                        Type::union(self, u, ns_id)
                                    }
                                };

                                fields.push(Field {
                                    name: field_name,
                                    typ: type_id,
                                    doc: u_doc,
                                    c_type: ctype,
                                    ..Field::default()
                                });
                                union_count += 1;
                            }
                        }
                        "field" => {
                            if let Ok(mut f) = self.read_field(parser, ns_id, &attributes) {
                                // Workaround for wrong GValue c:type
                                if c_type == "GValue" && f.name == "data" {
                                    f.c_type = Some("GValue_data".into());
                                }
                                fields.push(f);
                            }
                        }
                        "doc" => doc = try!(read_text(parser)),
                        "doc-deprecated" => doc_deprecated = try!(read_text(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }

        if attrs.by_name("is-gtype-struct").is_some() {
            return Ok(None);
        }

        if record_name == "Atom" && self.namespace(ns_id).name == "Gdk" {
            // the gir definitions don't reflect the following correctly
            // typedef struct _GdkAtom *GdkAtom;
            record_name = "Atom_";
            c_type = "GdkAtom_";
            let tid = self.find_or_stub_type(ns_id, "Gdk.Atom_");
            self.add_type(
                ns_id,
                "Atom",
                Type::Alias(Alias {
                    name: "Atom".into(),
                    c_identifier: "GdkAtom".into(),
                    typ: tid,
                    target_c_type: "GdkAtom_*".into(),
                    doc: None, //TODO: temporary
                }),
            );
        }

        let typ = Type::Record(Record {
            name: record_name.into(),
            c_type: c_type.into(),
            glib_get_type: get_type.map(|s| s.into()),
            fields: fields,
            functions: fns,
            version: version,
            deprecated_version: deprecated_version,
            doc: doc,
            doc_deprecated: doc_deprecated,
        });

        Ok(Some(typ))
    }

    fn read_named_union(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
    ) -> Result<()> {
        // Require a name here
        try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing union name", parser))
        );
        if let Type::Union(mut u) = try!(self.read_union(parser, ns_id, attrs, None, None)) {
            assert_ne!(u.name, "");
            // Workaround for missing c:type
            if u.name == "_Value__data__union" {
                u.c_type = Some("GValue_data".into());
            } else if u.c_type.is_none() {
                return Err(mk_error!("Missing union c:type", parser).into());
            }
            let union_name = u.name.clone();
            self.add_type(ns_id, &union_name, Type::Union(u));
        }
        Ok(())
    }

    fn read_union(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
        parent_name_prefix: Option<&str>,
        parent_ctype_prefix: Option<&str>,
    ) -> Result<Type> {
        let union_name = attrs.by_name("name").unwrap_or("");
        let c_type = attrs.by_name("type").unwrap_or("");
        let get_type = attrs.by_name("get-type").map(|s| s.into());

        let mut fields = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut struct_count = 1;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "field" => {
                            let f = try!(self.read_field(parser, ns_id, &attributes));
                            fields.push(f);
                        }
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            try!(self.read_function_to_vec(
                                parser,
                                ns_id,
                                kind,
                                &attributes,
                                &mut fns,
                            ))
                        }
                        "record" => {
                            let mut r = match try!(self.read_record(
                                parser,
                                ns_id,
                                attrs,
                                parent_name_prefix,
                                parent_ctype_prefix
                            )) {
                                Some(Type::Record(r)) => r,
                                _ => continue,
                            };

                            let field_name = if let Some(field_name) = attributes.by_name("name") {
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

                            let type_id = {
                                #[cfg(not(feature = "use_unions"))]
                                {
                                    Type::record(self, r, INTERNAL_NAMESPACE)
                                }
                                #[cfg(feature = "use_unions")]
                                {
                                    Type::record(self, r, ns_id)
                                }
                            };

                            fields.push(Field {
                                name: field_name,
                                typ: type_id,
                                doc: r_doc,
                                c_type: Some(ctype),
                                ..Field::default()
                            });

                            struct_count += 1;
                        }
                        "doc" => doc = try!(read_text(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }

        let typ = Type::Union(Union {
            name: union_name.into(),
            c_type: Some(c_type.into()),
            glib_get_type: get_type,
            fields: fields,
            functions: fns,
            doc: doc,
        });
        Ok(typ)
    }

    fn read_field(&mut self, parser: &mut Reader, ns_id: u16, attrs: &Attributes) -> Result<Field> {
        let field_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing field name", parser))
        );
        let mut typ = None;
        let mut doc = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            let pos = parser.position();
                            if typ.is_some() {
                                bail!(mk_error!("Too many <type> elements", &pos));
                            }
                            typ = Some(try!(self.read_type(parser, ns_id, &name, &attributes)));
                        }
                        "callback" => {
                            let pos = parser.position();
                            if typ.is_some() {
                                bail!(mk_error!("Too many <type> elements", &pos));
                            }
                            let f =
                                try!(self.read_function(parser, ns_id, "callback", &attributes));
                            typ = Some((Type::function(self, f), None, None));
                        }
                        "doc" => doc = try!(read_text(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        let private = attrs.by_name("private").unwrap_or("") == "1";
        let bits = attrs.by_name("bits").and_then(|s| s.parse().ok());
        if let Some((tid, c_type, array_length)) = typ {
            Ok(Field {
                name: field_name.into(),
                typ: tid,
                c_type: c_type,
                private: private,
                bits: bits,
                array_length: array_length,
                doc: doc,
            })
        } else {
            bail!(mk_error!("Missing <type> element", parser))
        }
    }

    fn read_named_callback(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
    ) -> Result<()> {
        try!(self.read_function_if_not_moved(
            parser,
            ns_id,
            "callback",
            attrs,
        )).map(|func| {
            self.add_type(ns_id, &func.name.clone(), Type::Function(func))
        });

        Ok(())
    }

    fn read_interface(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
    ) -> Result<()> {
        let interface_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing interface name", parser))
        );
        let c_type = try!(
            attrs
                .by_name("type")
                .ok_or_else(|| mk_error!("Missing c:type attribute", parser))
        );
        let get_type = try!(
            attrs
                .by_name("get-type")
                .ok_or_else(|| mk_error!("Missing get-type attribute", parser))
        );
        let version = try!(self.parse_version(parser, ns_id, attrs.by_name("version")));
        let deprecated_version = try!(self.parse_version(
            parser,
            ns_id,
            attrs.by_name("deprecated-version"),
        ));
        let mut fns = Vec::new();
        let mut signals = Vec::new();
        let mut properties = Vec::new();
        let mut prereqs = Vec::new();
        let mut doc = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            try!(self.read_function_to_vec(
                                parser,
                                ns_id,
                                kind,
                                &attributes,
                                &mut fns,
                            ))
                        }
                        "prerequisite" => {
                            prereqs.push(try!(self.read_type(parser, ns_id, &name, &attributes)).0)
                        }
                        "signal" => {
                            try!(self.read_signal_to_vec(
                                parser,
                                ns_id,
                                &attributes,
                                &mut signals,
                            ))
                        }
                        "property" => {
                            try!(self.read_property_to_vec(
                                parser,
                                ns_id,
                                &attributes,
                                &mut properties,
                            ))
                        }
                        "doc" => doc = try!(read_text(parser)),
                        _ => try!(ignore_element(parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }

        let typ = Type::Interface(Interface {
            name: interface_name.into(),
            c_type: c_type.into(),
            glib_get_type: get_type.into(),
            functions: fns,
            signals: signals,
            properties: properties,
            prerequisites: prereqs,
            doc: doc,
            version: version,
            deprecated_version: deprecated_version,
        });
        self.add_type(ns_id, interface_name, typ);
        Ok(())
    }

    fn read_bitfield(&mut self, parser: &mut Reader, ns_id: u16, attrs: &Attributes) -> Result<()> {
        let bitfield_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing bitfield name", parser))
        );
        let c_type = try!(
            attrs
                .by_name("type")
                .ok_or_else(|| mk_error!("Missing c:type attribute", parser))
        );
        let get_type = attrs.by_name("get-type").map(|s| s.into());
        let version = try!(self.parse_version(parser, ns_id, attrs.by_name("version")));
        let deprecated_version = try!(self.parse_version(
            parser,
            ns_id,
            attrs.by_name("deprecated-version"),
        ));
        let mut members = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "member" => {
                            members.push(try!(self.read_member(parser, &attributes)));
                        }
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            try!(self.read_function_to_vec(
                                parser,
                                ns_id,
                                kind,
                                &attributes,
                                &mut fns,
                            ))
                        }
                        "doc" => doc = try!(read_text(parser)),
                        "doc-deprecated" => doc_deprecated = try!(read_text(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }

        let typ = Type::Bitfield(Bitfield {
            name: bitfield_name.into(),
            c_type: c_type.into(),
            members: members,
            functions: fns,
            version: version,
            deprecated_version: deprecated_version,
            doc: doc,
            doc_deprecated: doc_deprecated,
            glib_get_type: get_type,
        });
        self.add_type(ns_id, bitfield_name, typ);
        Ok(())
    }

    fn read_enumeration(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
    ) -> Result<()> {
        let enum_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing enumeration name", parser))
        );
        let c_type = try!(
            attrs
                .by_name("type")
                .ok_or_else(|| mk_error!("Missing c:type attribute", parser))
        );
        let get_type = attrs.by_name("get-type").map(|s| s.into());
        let version = try!(self.parse_version(parser, ns_id, attrs.by_name("version")));
        let deprecated_version = try!(self.parse_version(
            parser,
            ns_id,
            attrs.by_name("deprecated-version"),
        ));
        let error_domain = attrs.by_name("error-domain").map(String::from);
        let mut members = Vec::new();
        let mut fns = Vec::new();
        let mut doc = None;
        let mut doc_deprecated = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "member" => {
                            members.push(try!(self.read_member(parser, &attributes)));
                        }
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            try!(self.read_function_to_vec(
                                parser,
                                ns_id,
                                kind,
                                &attributes,
                                &mut fns,
                            ))
                        }
                        "doc" => doc = try!(read_text(parser)),
                        "doc-deprecated" => doc_deprecated = try!(read_text(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }

        let typ = Type::Enumeration(Enumeration {
            name: enum_name.into(),
            c_type: c_type.into(),
            members: members,
            functions: fns,
            version: version,
            deprecated_version: deprecated_version,
            doc: doc,
            doc_deprecated: doc_deprecated,
            error_domain: error_domain,
            glib_get_type: get_type,
        });
        self.add_type(ns_id, enum_name, typ);
        Ok(())
    }

    fn read_global_function(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
    ) -> Result<()> {
        try!(self.read_function_if_not_moved(
            parser,
            ns_id,
            "global",
            attrs,
        )).map(|func| self.add_function(ns_id, func));

        Ok(())
    }

    fn read_constant(&mut self, parser: &mut Reader, ns_id: u16, attrs: &Attributes) -> Result<()> {
        let const_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing constant name", parser))
        );
        let c_identifier = try!(
            attrs
                .by_name("type")
                .ok_or_else(|| mk_error!("Missing c:type attribute", parser))
        );
        let value = try!(
            attrs
                .by_name("value")
                .ok_or_else(|| mk_error!("Missing constant value", parser))
        );
        let version = try!(self.parse_version(parser, ns_id, attrs.by_name("version")));
        let deprecated_version = try!(self.parse_version(
            parser,
            ns_id,
            attrs.by_name("deprecated-version"),
        ));
        let mut inner = None;
        let mut doc = None;
        let mut doc_deprecated = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            if inner.is_some() {
                                bail!(mk_error!("Too many <type> elements", parser));
                            }
                            let pos = parser.position();
                            let (typ, c_type, array_length) =
                                try!(self.read_type(parser, ns_id, &name, &attributes));
                            if let Some(c_type) = c_type {
                                inner = Some((typ, c_type, array_length));
                            } else {
                                bail!(mk_error!("Missing constant's c:type", &pos));
                            }
                        }
                        "doc" => doc = try!(read_text(parser)),
                        "doc-deprecated" => doc_deprecated = try!(read_text(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        if let Some((typ, c_type, _array_length)) = inner {
            self.add_constant(
                ns_id,
                Constant {
                    name: const_name.into(),
                    c_identifier: c_identifier.into(),
                    typ: typ,
                    c_type: c_type.into(),
                    value: value.into(),
                    version: version,
                    deprecated_version: deprecated_version,
                    doc: doc,
                    doc_deprecated: doc_deprecated,
                },
            );
            Ok(())
        } else {
            bail!(mk_error!("Missing <type> element", parser))
        }
    }

    fn read_alias(&mut self, parser: &mut Reader, ns_id: u16, attrs: &Attributes) -> Result<()> {
        let alias_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing alias name", parser))
        );
        let c_identifier = try!(
            attrs
                .by_name("type")
                .ok_or_else(|| mk_error!("Missing c:type attribute", parser))
        );
        let mut inner = None;
        let mut doc = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            if inner.is_some() {
                                bail!(mk_error!("Too many <type> elements", parser));
                            }
                            let pos = parser.position();
                            let (typ, c_type, array_length) =
                                try!(self.read_type(parser, ns_id, &name, &attributes));
                            if let Some(c_type) = c_type {
                                inner = Some((typ, c_type, array_length));
                            } else {
                                bail!(mk_error!("Missing alias target's c:type", &pos));
                            }
                        }
                        "doc" => doc = try!(read_text(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        if let Some((typ, c_type, _array_length)) = inner {
            let typ = Type::Alias(Alias {
                name: alias_name.into(),
                c_identifier: c_identifier.into(),
                typ: typ,
                target_c_type: c_type.into(),
                doc: doc,
            });
            self.add_type(ns_id, alias_name, typ);
            Ok(())
        } else {
            bail!(mk_error!("Missing <type> element", parser))
        }
    }

    fn read_member(&self, parser: &mut Reader, attrs: &Attributes) -> Result<Member> {
        let member_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing member name", parser))
        );
        let value = try!(
            attrs
                .by_name("value")
                .ok_or_else(|| mk_error!("Missing member value", parser))
        );
        let c_identifier = attrs.by_name("identifier").map(|x| x.into());
        let mut doc = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match (name.local_name.as_ref(), attributes.by_name("name")) {
                        /*
                        ("attribute", Some("c:identifier")) => {
                            let value = try!(attributes.get("value")
                                .ok_or_else(|| mk_error!("Missing attribute value", parser)));
                            c_identifier = Some(value.into());
                        }
                        */
                        ("doc", _) => doc = try!(read_text(parser)),
                        (x, _) => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        Ok(Member {
            name: member_name.into(),
            value: value.into(),
            doc: doc,
            c_identifier: c_identifier.unwrap_or_else(|| member_name.into()),
        })
    }

    fn read_function(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        kind_str: &str,
        attrs: &Attributes,
    ) -> Result<Function> {
        let fn_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing function name", parser))
        );
        let c_identifier = attrs
            .by_name("identifier")
            .or_else(|| attrs.by_name("type"));
        let kind = try!(FunctionKind::from_str(kind_str).map_err(|why| mk_error!(why, parser)));
        let is_method = kind == FunctionKind::Method;
        let version = try!(self.parse_version(parser, ns_id, attrs.by_name("version")));
        let deprecated_version = try!(self.parse_version(
            parser,
            ns_id,
            attrs.by_name("deprecated-version"),
        ));
        let mut params = Vec::new();
        let mut ret = None;
        let mut doc = None;
        let mut doc_deprecated = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "parameters" => {
                            //params.append(&mut try!(self.read_parameters(parser, ns_id)));
                            try!(self.read_parameters(parser, ns_id, false, is_method))
                                .into_iter()
                                .map(|p| params.push(p))
                                .count();
                        }
                        "return-value" => {
                            if ret.is_some() {
                                bail!(mk_error!("Too many <return-value> elements", parser));
                            }
                            ret = Some(try!(self.read_parameter(
                                parser,
                                ns_id,
                                "return-value",
                                &attributes,
                                false,
                                is_method
                            )));
                        }
                        "doc" => doc = try!(read_text(parser)),
                        "doc-deprecated" => doc_deprecated = try!(read_text(parser)),
                        "doc-version" => try!(ignore_element(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        let throws = attrs.by_name("throws").unwrap_or("") == "1";
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
            });
        }
        if let Some(ret) = ret {
            Ok(Function {
                name: fn_name.into(),
                c_identifier: c_identifier.map(|s| s.into()),
                kind: kind,
                parameters: params,
                ret: ret,
                throws: throws,
                version: version,
                deprecated_version: deprecated_version,
                doc: doc,
                doc_deprecated: doc_deprecated,
            })
        } else {
            bail!(mk_error!("Missing <return-value> element", parser))
        }
    }

    fn read_function_to_vec(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        kind_str: &str,
        attrs: &Attributes,
        fns: &mut Vec<Function>,
    ) -> Result<()> {
        try!(self.read_function_if_not_moved(
            parser,
            ns_id,
            kind_str,
            attrs,
        )).map(|f| fns.push(f));

        Ok(())
    }

    fn read_function_if_not_moved(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        kind_str: &str,
        attrs: &Attributes,
    ) -> Result<Option<Function>> {
        let moved_to = attrs.by_name("moved-to").is_some();
        if moved_to {
            try!(ignore_element(parser));
            return Ok(None);
        }
        let pos = parser.position();
        let f = try!(self.read_function(parser, ns_id, kind_str, attrs));
        if f.c_identifier.is_none() {
            bail!(mk_error!("Missing c:identifier attribute", &pos));
        }

        Ok(Some(f))
    }

    fn read_signal(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
    ) -> Result<Signal> {
        let signal_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing signal name", parser))
        );
        let is_action = to_bool(attrs.by_name("action").unwrap_or("none"));
        let version = match attrs.by_name("version") {
            Some(v) => Some(try!(v.parse().map_err(|why| mk_error!(why, parser)))),
            None => None,
        };
        let deprecated = to_bool(attrs.by_name("deprecated").unwrap_or("none"));
        let deprecated_version = if deprecated {
            match attrs.by_name("deprecated-version") {
                Some(v) => Some(try!(v.parse().map_err(|why| mk_error!(why, parser)))),
                None => None,
            }
        } else {
            None
        };
        if let Some(v) = version {
            self.register_version(ns_id, v);
        }
        if let Some(v) = deprecated_version {
            self.register_version(ns_id, v);
        }

        let mut params = Vec::new();
        let mut ret = None;
        let mut doc = None;
        let mut doc_deprecated = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "parameters" => {
                            try!(self.read_parameters(parser, ns_id, true, false))
                                .into_iter()
                                .map(|p| params.push(p))
                                .count();
                        }
                        "return-value" => {
                            if ret.is_some() {
                                bail!(mk_error!("Too many <return-value> elements", parser));
                            }
                            ret = Some(try!(self.read_parameter(
                                parser,
                                ns_id,
                                "return-value",
                                &attributes,
                                true,
                                false
                            )));
                        }
                        "doc" => doc = try!(read_text(parser)),
                        "doc-deprecated" => doc_deprecated = try!(read_text(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        if let Some(ret) = ret {
            Ok(Signal {
                name: signal_name.into(),
                parameters: params,
                ret: ret,
                is_action: is_action,
                version: version,
                deprecated_version: deprecated_version,
                doc: doc,
                doc_deprecated: doc_deprecated,
            })
        } else {
            bail!(mk_error!("Missing <return-value> element", parser))
        }
    }

    fn read_signal_to_vec(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
        signals: &mut Vec<Signal>,
    ) -> Result<()> {
        let s = try!(self.read_signal(parser, ns_id, attrs));
        signals.push(s);

        Ok(())
    }

    fn read_parameters(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        allow_no_ctype: bool,
        for_method: bool,
    ) -> Result<Vec<Parameter>> {
        let mut params = Vec::new();
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        kind @ "parameter" | kind @ "instance-parameter" => {
                            let param = try!(self.read_parameter(
                                parser,
                                ns_id,
                                kind,
                                &attributes,
                                allow_no_ctype,
                                for_method
                            ));
                            params.push(param);
                        }
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        Ok(params)
    }

    fn read_parameter(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        kind_str: &str,
        attrs: &Attributes,
        allow_no_ctype: bool,
        for_method: bool,
    ) -> Result<Parameter> {
        let param_name = attrs.by_name("name").unwrap_or("");
        let instance_parameter = kind_str == "instance-parameter";
        let transfer = try!(
            Transfer::from_str(attrs.by_name("transfer-ownership").unwrap_or("none"))
                .map_err(|why| mk_error!(why, parser))
        );
        let nullable = to_bool(attrs.by_name("nullable").unwrap_or("none"));
        let allow_none = to_bool(attrs.by_name("allow-none").unwrap_or("none"));
        let caller_allocates = to_bool(attrs.by_name("caller-allocates").unwrap_or("none"));
        let direction = try!(if kind_str == "return-value" {
            Ok(ParameterDirection::Return)
        } else {
            ParameterDirection::from_str(attrs.by_name("direction").unwrap_or("in"))
                .map_err(|why| mk_error!(why, parser))
        });

        let mut typ = None;
        let mut varargs = false;
        let mut doc = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            let pos = parser.position();
                            if typ.is_some() {
                                bail!(mk_error!("Too many <type> elements", &pos));
                            }
                            typ = Some(try!(self.read_type(parser, ns_id, &name, &attributes)));
                            if let Some((tid, None, _)) = typ {
                                if allow_no_ctype {
                                    typ = Some((tid, Some(EMPTY_CTYPE.to_owned()), None));
                                } else {
                                    bail!(mk_error!("Missing c:type attribute", &pos));
                                }
                            }
                        }
                        "varargs" => {
                            varargs = true;
                            try!(ignore_element(parser));
                        }
                        "doc" => doc = try!(read_text(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        if let Some((tid, c_type, mut array_length)) = typ {
            if for_method {
                array_length = array_length.map(|l| l + 1);
            }
            Ok(Parameter {
                name: param_name.into(),
                typ: tid,
                c_type: c_type.unwrap(),
                instance_parameter: instance_parameter,
                direction: direction,
                transfer: transfer,
                caller_allocates: caller_allocates,
                nullable: Nullable(nullable),
                allow_none: allow_none,
                array_length: array_length,
                is_error: false,
                doc: doc,
            })
        } else if varargs {
            Ok(Parameter {
                name: "".into(),
                typ: self.find_type(INTERNAL_NAMESPACE, "varargs").unwrap(),
                c_type: "".into(),
                instance_parameter: instance_parameter,
                direction: Default::default(),
                transfer: Transfer::None,
                caller_allocates: false,
                nullable: Nullable(false),
                allow_none: allow_none,
                array_length: None,
                is_error: false,
                doc: doc,
            })
        } else {
            bail!(mk_error!("Missing <type> element", parser))
        }
    }

    fn read_property(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
    ) -> Result<Option<Property>> {
        let prop_name = try!(
            attrs
                .by_name("name")
                .ok_or_else(|| mk_error!("Missing property name", parser))
        );
        let readable = to_bool(attrs.by_name("readable").unwrap_or("1"));
        let writable = to_bool(attrs.by_name("writable").unwrap_or("none"));
        let construct = to_bool(attrs.by_name("construct").unwrap_or("none"));
        let construct_only = to_bool(attrs.by_name("construct-only").unwrap_or("none"));
        let transfer = try!(
            Transfer::from_str(attrs.by_name("transfer-ownership").unwrap_or("none"))
                .map_err(|why| mk_error!(why, parser))
        );
        let version = match attrs.by_name("version") {
            Some(v) => Some(try!(v.parse().map_err(|why| mk_error!(why, parser)))),
            None => None,
        };
        let deprecated = to_bool(attrs.by_name("deprecated").unwrap_or("none"));
        let deprecated_version = if deprecated {
            match attrs.by_name("deprecated-version") {
                Some(v) => Some(try!(v.parse().map_err(|why| mk_error!(why, parser)))),
                None => None,
            }
        } else {
            None
        };
        if let Some(v) = version {
            self.register_version(ns_id, v);
        }
        if let Some(v) = deprecated_version {
            self.register_version(ns_id, v);
        }
        let mut has_empty_type_tag = false;
        let mut typ = None;
        let mut doc = None;
        let mut doc_deprecated = None;
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            let pos = parser.position();
                            if typ.is_some() {
                                bail!(mk_error!("Too many <type> elements", &pos));
                            }
                            //defend from <type/>
                            if attributes.is_empty() && name.local_name == "type" {
                                try!(ignore_element(parser));
                                has_empty_type_tag = true;
                                continue;
                            }
                            typ = Some(try!(self.read_type(parser, ns_id, &name, &attributes)));
                            if let Some((tid, None, _)) = typ {
                                typ = Some((tid, Some(EMPTY_CTYPE.to_owned()), None));
                            }
                        }
                        "doc" => doc = try!(read_text(parser)),
                        "doc-deprecated" => doc_deprecated = try!(read_text(parser)),
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        if has_empty_type_tag {
            return Ok(None);
        }

        if let Some((tid, c_type, _array_length)) = typ {
            Ok(Some(Property {
                name: prop_name.into(),
                readable: readable,
                writable: writable,
                construct: construct,
                construct_only: construct_only,
                transfer: transfer,
                typ: tid,
                c_type: c_type,
                version: version,
                deprecated_version: deprecated_version,
                doc: doc,
                doc_deprecated: doc_deprecated,
            }))
        } else {
            bail!(mk_error!("Missing <type> element", parser))
        }
    }

    fn read_property_to_vec(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attrs: &Attributes,
        properties: &mut Vec<Property>,
    ) -> Result<()> {
        let s = try!(self.read_property(parser, ns_id, attrs));
        if let Some(s) = s {
            properties.push(s);
        }

        Ok(())
    }

    fn read_type(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        name: &OwnedName,
        attrs: &Attributes,
    ) -> Result<(TypeId, Option<String>, Option<u32>)> {
        let start_pos = parser.position();
        let type_name = try!(
            attrs
                .by_name("name")
                .or_else(|| if name.local_name == "array" {
                    Some("array")
                } else {
                    None
                })
                .ok_or_else(|| mk_error!("Missing type name", &start_pos))
        );
        let c_type = attrs.by_name("type").map(|s| s.into());
        let array_length = attrs.by_name("length").and_then(|s| s.parse().ok());
        let mut inner = Vec::new();
        loop {
            let event = try!(parser.next());
            match event {
                StartElement {
                    name, attributes, ..
                } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            inner.push(try!(self.read_type(parser, ns_id, &name, &attributes)).0);
                        }
                        x => bail!(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_next!(event, parser),
            }
        }
        if inner.is_empty() || type_name == "GLib.ByteArray" {
            if type_name == "array" {
                bail!(mk_error!("Missing element type", &start_pos))
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
                    attrs.by_name("fixed-size").and_then(|n| n.parse().ok()),
                )
            } else {
                try!(
                    Type::container(self, type_name, inner)
                        .ok_or_else(|| mk_error!("Unknown container type", &start_pos))
                )
            };
            Ok((tid, c_type, array_length))
        }
    }

    fn parse_version(
        &mut self,
        parser: &mut Reader,
        ns_id: u16,
        attr: Option<&str>,
    ) -> Result<Option<Version>> {
        let ret = match attr {
            Some(v) => Ok(Some(try!(v.parse().map_err(|why| mk_error!(why, parser))))),
            None => Ok(None),
        };
        if let Ok(Some(version)) = ret {
            self.register_version(ns_id, version);
        }
        ret
    }
}

trait ByName {
    fn by_name<'a>(&'a self, name: &str) -> Option<&'a str>;
}

impl ByName for Attributes {
    fn by_name<'a>(&'a self, name: &str) -> Option<&'a str> {
        for attr in self {
            if attr.name.local_name == name {
                return Some(&attr.value);
            }
        }
        None
    }
}

fn read_text(parser: &mut Reader) -> Result<Option<String>> {
    let mut ret_text = None;

    loop {
        let event = try!(parser.next());
        match event {
            Characters(text) => {
                ret_text = match ret_text {
                    Some(t) => Some(format!("{}{}", t, text)),
                    None => Some(text),
                }
            }
            EndElement { .. } => break,
            StartElement { name, .. } => {
                bail!(mk_error!(
                    &format!("Unexpected element: {}", name.local_name),
                    parser
                ))
            }
            _ => xml_next!(event, parser),
        }
    }
    Ok(ret_text)
}

fn ignore_element(parser: &mut Reader) -> Result<()> {
    loop {
        let event = try!(parser.next());
        match event {
            StartElement { .. } => try!(ignore_element(parser)),
            EndElement { .. } => return Ok(()),
            _ => xml_next!(event, parser),
        }
    }
}

fn make_file_name(dir: &Path, name: &str) -> PathBuf {
    let mut path = dir.to_path_buf();
    let name = format!("{}.gir", name);
    path.push(name);
    path
}

fn to_bool(s: &str) -> bool {
    s == "1"
}
