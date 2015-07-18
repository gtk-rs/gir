use std::io::BufReader;
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;
use xml::attribute::OwnedAttribute;
use xml::common::{Error, Position};
use xml::name::OwnedName;
use xml::reader::EventReader;
use xml::reader::events::XmlEvent::{self, StartElement, EndElement, EndDocument};

use library::*;

type Reader = EventReader<BufReader<File>>;
type Attributes = Vec<OwnedAttribute>;

macro_rules! mk_error {
    ($msg:expr, $obj:expr) => (
        Error::new($obj, format!("{}:{} {}", file!(), line!(), $msg))
    )
}

macro_rules! xml_try {
    ($event:expr, $pos:expr) => (
        match $event {
            XmlEvent::Error(e) => return Err(e),
            EndDocument => return Err(mk_error!("Unexpected end of document", $pos)),
            _ => (),
        }
    )
}

impl Library {
    pub fn read_file(&mut self, dir: &str, lib: &str) {
        let name = make_file_name(dir, lib);
        let display_name = name.to_string_lossy().into_owned();
        let file = BufReader::new(File::open(&name)
            .unwrap_or_else(|e| panic!("{} - {}", e, name.to_string_lossy())));
        let mut parser = EventReader::new(file);
        loop {
            let event = parser.next();
            match event {
                StartElement { name: OwnedName { ref local_name,
                                                 namespace: Some(ref namespace), .. }, .. }
                            if local_name == &"repository"
                            && namespace == &"http://www.gtk.org/introspection/core/1.0" => {
                    match self.read_repository(dir, &mut parser) {
                        Err(e) => panic!("{} in {}:{}",
                                         e.msg(), display_name, e.position()),
                        Ok(_) => (),
                    }
                }
                XmlEvent::Error(e) => panic!("{} in {}:{}",
                                             e.msg(), display_name, e.position()),
                EndDocument => break,
                _ => continue,
            }
        }
    }

    fn read_repository(&mut self, dir: &str, parser: &mut Reader) -> Result<(), Error> {
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        "include" => {
                            if let (Some(lib), Some(ver)) =
                                (attributes.get("name"), attributes.get("version")) {
                                if self.find_namespace(lib).is_none() {
                                    let lib = format!("{}-{}", lib, ver);
                                    self.read_file(dir, &lib);
                                }
                            }
                            try!(ignore_element(parser));
                        }
                        "namespace" => try!(self.read_namespace(parser, &attributes)),
                        _ => try!(ignore_element(parser)),
                    }
                }
                EndElement { .. } => return Ok(()),
                _ => xml_try!(event, parser),
            }
        }
    }

    fn read_namespace(&mut self, parser: &mut Reader,
                      attrs: &Attributes) -> Result<(), Error> {
        let name = try!(attrs.get("name").ok_or_else(|| mk_error!("Missing namespace name", parser)));
        let ns_id = self.add_namespace(name);
        trace!("Reading {}-{}", name, attrs.get("version").unwrap());
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    trace!("<{} name={:?}>", name.local_name, attributes.get("name"));
                    match name.local_name.as_ref() {
                        "class" => {
                            trace!("class {}", attributes.get("name").unwrap());
                            try!(self.read_class(parser, ns_id, &attributes));
                        }
                        "record" => {
                            try!(self.read_record(parser, ns_id, &attributes));
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
                            warn!("<{} name={:?}>", name.local_name, attributes.get("name"));
                            try!(ignore_element(parser));
                        }
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }
        Ok(())
    }

    fn read_class(&mut self, parser: &mut Reader,
                  ns_id: u16, attrs: &Attributes) -> Result<(), Error> {
        let name = try!(attrs.get("name").ok_or_else(|| mk_error!("Missing class name", parser)));
        let c_type = try!(attrs.get("type").or_else(|| attrs.get("type-name"))
            .ok_or_else(|| mk_error!("Missing c:type/glib:type-name attributes", parser)));
        let get_type = try!(attrs.get("get-type")
            .ok_or_else(|| mk_error!("Missing get-type attribute", parser)));
        let mut fns = Vec::new();
        let mut impls = Vec::new();
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            let pos = parser.position();
                            let f = try!(self.read_function(parser, ns_id, kind, &attributes));
                            if f.c_identifier.is_none() {
                                return Err(mk_error!("Missing c:identifier attribute", &pos));
                            }
                            fns.push(f);
                        }
                        "implements" => {
                            impls.push(try!(self.read_type(parser, ns_id, &name, &attributes)).0);
                        }
                        "field" | "property"
                            | "signal" | "virtual-method" => try!(ignore_element(parser)),
                        "doc" | "doc-deprecated" => try!(ignore_element(parser)),
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }

        let parent = attrs.get("parent").map(|s| self.find_or_stub_type(ns_id, s));
        let typ = Type::Class(
            Class {
                name: name.into(),
                c_type: c_type.into(),
                glib_get_type : get_type.into(),
                functions: fns,
                parent: parent,
                implements: impls,
                .. Class::default()
            });
        self.add_type(ns_id, name, typ);
        Ok(())
    }

    fn read_record(&mut self, parser: &mut Reader,
                  ns_id: u16, attrs: &Attributes) -> Result<(), Error> {
        let name = try!(attrs.get("name").ok_or_else(|| mk_error!("Missing record name", parser)));
        let c_type = try!(attrs.get("type")
                          .ok_or_else(|| mk_error!("Missing c:type attribute", parser)));
        let mut fields = Vec::new();
        let mut fns = Vec::new();
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            let pos = parser.position();
                            let f = try!(self.read_function(parser, ns_id, kind, &attributes));
                            if f.c_identifier.is_none() {
                                return Err(mk_error!("Missing c:identifier attribute", &pos));
                            }
                            fns.push(f);
                        }
                        "union" => {
                            let (union_fields, _) = try!(self.read_union(parser, ns_id));
                            let typ = Type::union(self, union_fields);
                            fields.push(Field { typ: typ, .. Field::default() });
                        }
                        "field" => {
                            fields.push(try!(
                                self.read_field(parser, ns_id, &attributes)));
                        }
                        "doc" | "doc-deprecated" => try!(ignore_element(parser)),
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }

        if attrs.get("is-gtype-struct").is_some() {
            return Ok(());
        }

        let typ = Type::Record(
            Record {
                name: name.into(),
                c_type: c_type.into(),
                fields: fields,
                functions: fns,
            });
        self.add_type(ns_id, name, typ);
        Ok(())
    }

    fn read_named_union(&mut self, parser: &mut Reader, ns_id: u16, attrs: &Attributes)
            -> Result<(), Error> {
        let name = try!(attrs.get("name").ok_or_else(|| mk_error!("Missing union name", parser)));
        let c_type = attrs.get("type");
        let (fields, fns) = try!(self.read_union(parser, ns_id));
        let typ = Type::Union(
            Union {
                name: name.into(),
                c_type: c_type.map(|s| s.into()),
                fields: fields,
                functions: fns,
            });
        self.add_type(ns_id, name, typ);
        Ok(())
    }

    fn read_union(&mut self, parser: &mut Reader, ns_id: u16)
            -> Result<(Vec<Field>, Vec<Function>), Error> {
        let mut fields = Vec::new();
        let mut fns = Vec::new();
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        "field" => {
                            fields.push(try!(
                                self.read_field(parser, ns_id, &attributes)));
                        }
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            let pos = parser.position();
                            let f = try!(self.read_function(parser, ns_id, kind, &attributes));
                            if f.c_identifier.is_none() {
                                return Err(mk_error!("Missing c:identifier attribute", &pos));
                            }
                            fns.push(f);
                        }
                        "record" => try!(ignore_element(parser)),
                        "doc" | "doc-deprecated" => try!(ignore_element(parser)),
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }
        Ok((fields, fns))
    }

    fn read_field(&mut self, parser: &mut Reader, ns_id: u16,
                  attrs: &Attributes) -> Result<Field, Error> {
        let name = try!(attrs.get("name").ok_or_else(|| mk_error!("Missing field name", parser)));
        let mut typ = None;
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            let pos = parser.position();
                            if typ.is_some() {
                                return Err(mk_error!("Too many <type> elements", &pos));
                            }
                            typ = Some(try!(self.read_type(parser, ns_id, &name, &attributes)));
                        }
                        "callback" => {
                            let pos = parser.position();
                            if typ.is_some() {
                                return Err(mk_error!("Too many <type> elements", &pos));
                            }
                            let f =
                                try!(self.read_function(parser, ns_id, "callback", &attributes));
                            typ = Some((Type::function(self, f), None));
                        }
                        "doc" | "doc-deprecated" => try!(ignore_element(parser)),
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }
        let private = attrs.get("private").unwrap_or("") == "1";
        let bits = attrs.get("bits").and_then(|s| s.parse().ok());
        if let Some((tid, c_type)) = typ {
            Ok(Field {
                name: name.into(),
                typ: tid,
                c_type: c_type,
                private: private,
                bits: bits,
            })
        }
        else {
            Err(mk_error!("Missing <type> element", parser))
        }
    }

    fn read_named_callback(&mut self, parser: &mut Reader, ns_id: u16,
                     attrs: &Attributes) -> Result<(), Error> {
        let pos = parser.position();
        let func = try!(self.read_function(parser, ns_id, "callback", attrs));
        let name = func.name.clone();
        if func.c_identifier.is_none() {
            return Err(mk_error!("Missing c:type attribute", &pos));
        }
        self.add_type(ns_id, &name, Type::Function(func));
        Ok(())
    }

    fn read_interface(&mut self, parser: &mut Reader,
                      ns_id: u16, attrs: &Attributes) -> Result<(), Error> {
        let name = try!(attrs.get("name").ok_or_else(|| mk_error!("Missing interface name", parser)));
        let c_type = try!(attrs.get("type")
                          .ok_or_else(|| mk_error!("Missing c:type attribute", parser)));
        let get_type = try!(attrs.get("get-type")
            .ok_or_else(|| mk_error!("Missing get-type attribute", parser)));
        let mut fns = Vec::new();
        let mut prereqs = Vec::new();
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            let pos = parser.position();
                            let f = try!(self.read_function(parser, ns_id, kind, &attributes));
                            if f.c_identifier.is_none() {
                                return Err(mk_error!("Missing c:identifier attribute", &pos));
                            }
                            fns.push(f);
                        }
                        "prerequisite" => {
                            prereqs.push(try!(self.read_type(parser, ns_id, &name, &attributes)).0);
                        }
                        "doc" | "doc-deprecated" => try!(ignore_element(parser)),
                        _ => try!(ignore_element(parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }

        let typ = Type::Interface(
            Interface {
                name: name.into(),
                c_type: c_type.into(),
                glib_get_type : get_type.into(),
                functions: fns,
                prerequisites: prereqs,
                .. Interface::default()
            });
        self.add_type(ns_id, name, typ);
        Ok(())
    }

    fn read_bitfield(&mut self, parser: &mut Reader, ns_id: u16,
                     attrs: &Attributes) -> Result<(), Error> {
        let name = try!(attrs.get("name")
                        .ok_or_else(|| mk_error!("Missing bitfield name", parser)));
        let c_type = try!(attrs.get("type")
                          .ok_or_else(|| mk_error!("Missing c:type attribute", parser)));
        let mut members = Vec::new();
        let mut fns = Vec::new();
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        "member" => {
                            members.push(try!(
                                self.read_member(parser, &attributes)));
                        }
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            let pos = parser.position();
                            let f = try!(self.read_function(parser, ns_id, kind, &attributes));
                            if f.c_identifier.is_none() {
                                return Err(mk_error!("Missing c:identifier attribute", &pos));
                            }
                            fns.push(f);
                        }
                        "doc" | "doc-deprecated" => try!(ignore_element(parser)),
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }

        let typ = Type::Bitfield(
            Bitfield {
                name: name.into(),
                c_type: c_type.into(),
                members: members,
                functions: fns,
            });
        self.add_type(ns_id, name, typ);
        Ok(())
    }

    fn read_enumeration(&mut self, parser: &mut Reader, ns_id: u16,
                        attrs: &Attributes) -> Result<(), Error> {
        let name = try!(attrs.get("name")
                        .ok_or_else(|| mk_error!("Missing enumeration name", parser)));
        let c_type = try!(attrs.get("type")
                          .ok_or_else(|| mk_error!("Missing c:type attribute", parser)));
        let mut members = Vec::new();
        let mut fns = Vec::new();
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        "member" => {
                            members.push(try!(
                                self.read_member(parser, &attributes)));
                        }
                        kind @ "constructor" | kind @ "function" | kind @ "method" => {
                            let pos = parser.position();
                            let f = try!(self.read_function(parser, ns_id, kind, &attributes));
                            if f.c_identifier.is_none() {
                                return Err(mk_error!("Missing c:identifier attribute", &pos));
                            }
                            fns.push(f);
                        }
                        "doc" | "doc-deprecated" => try!(ignore_element(parser)),
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }

        let typ = Type::Enumeration(
            Enumeration {
                name: name.into(),
                c_type: c_type.into(),
                members: members,
                functions: fns,
            });
        self.add_type(ns_id, name, typ);
        Ok(())
    }

    fn read_global_function(&mut self, parser: &mut Reader, ns_id: u16,
                            attrs: &Attributes) -> Result<(), Error> {
        let moved_to = attrs.get("moved-to").is_some();
        if moved_to { return ignore_element(parser); }
        let pos = parser.position();
        let func = try!(self.read_function(parser, ns_id, "global", attrs));
        if func.c_identifier.is_none() {
            return Err(mk_error!("Missing c:identifier attribute", &pos));
        }
        self.add_function(ns_id, func);
        Ok(())
    }

    fn read_constant(&mut self, parser: &mut Reader, ns_id: u16,
                     attrs: &Attributes) -> Result<(), Error> {
        let name = try!(attrs.get("name").ok_or_else(|| mk_error!("Missing constant name", parser)));
        let value = try!(attrs.get("value").ok_or_else(|| mk_error!("Missing constant value", parser)));
        let mut typ = None;
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            if typ.is_some() {
                                return Err(mk_error!("Too many <type> elements", parser));
                            }
                            typ = Some(try!(self.read_type(parser, ns_id, &name, &attributes)).0);
                        }
                        "doc" | "doc-deprecated" => try!(ignore_element(parser)),
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }
        if let Some(typ) = typ {
            self.add_constant(ns_id,
                Constant {
                    name: name.into(),
                    typ: typ,
                    value: value.into(),
                });
            Ok(())
        }
        else {
            Err(mk_error!("Missing <type> element", parser))
        }
    }

    fn read_alias(&mut self, parser: &mut Reader, ns_id: u16,
                     attrs: &Attributes) -> Result<(), Error> {
        let name = try!(attrs.get("name")
                        .ok_or_else(|| mk_error!("Missing alias name", parser)));
        let c_identifier = try!(attrs.get("type")
                                .ok_or_else(|| mk_error!("Missing c:type attribute", parser)));
        let mut inner = None;
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            if inner.is_some() {
                                return Err(mk_error!("Too many <type> elements", parser));
                            }
                            let pos = parser.position();
                            let (typ, c_type) =
                                try!(self.read_type(parser, ns_id, &name, &attributes));
                            if let Some(c_type) = c_type {
                                inner = Some((typ, c_type));
                            }
                            else{
                                return Err(mk_error!("Missing alias target's c:type", &pos));
                            }
                        }
                        "doc" | "doc-deprecated" => try!(ignore_element(parser)),
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }
        if let Some((typ, c_type)) = inner {
            let typ = Type::Alias(
                Alias {
                    name: name.into(),
                    c_identifier: c_identifier.into(),
                    typ: typ,
                    target_c_type: c_type.into(),
                });
            self.add_type(ns_id, name, typ);
            Ok(())
        }
        else {
            Err(mk_error!("Missing <type> element", parser))
        }
    }

    fn read_member(&self, parser: &mut Reader, attrs: &Attributes) -> Result<Member, Error> {
        let name = try!(attrs.get("name").ok_or_else(|| mk_error!("Missing member name", parser)));
        let value = try!(attrs.get("value").ok_or_else(|| mk_error!("Missing member value", parser)));
        let c_identifier = attrs.get("identifier").map(|x| x.into());
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match (name.local_name.as_ref(), attributes.get("name")) {
                        /*
                        ("attribute", Some("c:identifier")) => {
                            let value = try!(attributes.get("value")
                                .ok_or_else(|| mk_error!("Missing attribute value", parser)));
                            c_identifier = Some(value.into());
                        }
                        */
                        ("doc", _) | ("doc-deprecated", _) => try!(ignore_element(parser)),
                        (x, _) => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }
        Ok(Member {
            name: name.into(),
            value: value.into(),
            c_identifier: c_identifier.unwrap_or_else(|| name.into()),
        })
    }

    fn read_function(&mut self, parser: &mut Reader, ns_id: u16,
                     kind_str: &str, attrs: &Attributes) -> Result<Function, Error> {
        let name = try!(attrs.get("name").ok_or_else(|| mk_error!("Missing function name", parser)));
        let c_identifier = attrs.get("identifier").or_else(|| attrs.get("type"));
        let kind = try!(FunctionKind::from_str(kind_str).map_err(|why| mk_error!(why, parser)));
        let mut params = Vec::new();
        let mut ret = None;
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        "parameters" => {
                            //params.append(&mut try!(self.read_parameters(parser, ns_id)));
                            try!(self.read_parameters(parser, ns_id)).into_iter()
                                .map(|p| params.push(p)).count();
                        }
                        "return-value" => {
                            if ret.is_some() {
                                return Err(mk_error!("Too many <return-value> elements", parser));
                            }
                            ret = Some(try!(
                                self.read_parameter(parser, ns_id, "return-value", &attributes)));
                        }
                        "doc" | "doc-deprecated" => try!(ignore_element(parser)),
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }
        let throws = attrs.get("throws").unwrap_or("") == "1";
        if throws {
            params.push(Parameter {
                name: "error".into(),
                typ: self.find_or_stub_type(ns_id, "GLib.Error"),
                c_type: "GError**".into(),
                instance_parameter: false,
                direction: ParameterDirection::Out,
                transfer: Transfer::Full,
                nullable: true,
                allow_none: true,
            });
        }
        if let Some(ret) = ret {
            Ok(Function {
                name: name.into(),
                c_identifier: c_identifier.map(|s| s.into()),
                kind: kind,
                parameters: params,
                ret: ret,
                throws: throws,
            })
        }
        else {
            Err(mk_error!("Missing <return-value> element", parser))
        }
    }

    fn read_parameters(&mut self, parser: &mut Reader, ns_id: u16)
                    -> Result<Vec<Parameter>, Error> {
        let mut params = Vec::new();
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        kind @ "parameter" | kind @ "instance-parameter"  => {
                            let param = try!(
                                self.read_parameter(parser, ns_id, kind, &attributes));
                            params.push(param);
                        }
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }
        Ok(params)
    }

    fn read_parameter(&mut self, parser: &mut Reader, ns_id: u16,
                      kind_str: &str, attrs: &Attributes) -> Result<Parameter, Error> {
        let name = attrs.get("name").unwrap_or("");
        let instance_parameter = kind_str == "instance-parameter";
        let transfer = try!(
            Transfer::from_str(attrs.get("transfer-ownership").unwrap_or("none"))
                .map_err(|why| mk_error!(why, parser)));
        let nullable = to_bool(attrs.get("nullable").unwrap_or("none"));
        let allow_none = to_bool(attrs.get("allow-none").unwrap_or("none"));
        let direction = try!(
            if kind_str == "return-value" {
                Ok(ParameterDirection::Return)
            } else {
                ParameterDirection::from_str(attrs.get("direction").unwrap_or("in"))
                    .map_err(|why| mk_error!(why, parser))
            });

        let mut typ = None;
        let mut varargs = false;
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            let pos = parser.position();
                            if typ.is_some() {
                                return Err(mk_error!("Too many <type> elements", &pos));
                            }
                            typ = Some(try!(self.read_type(parser, ns_id, &name, &attributes)));
                            if typ.as_ref().unwrap().1.is_none() {
                                return Err(mk_error!("Missing c:type attribute", &pos));
                            }
                        }
                        "varargs" => {
                            varargs = true;
                            try!(ignore_element(parser));
                        }
                        "doc" => try!(ignore_element(parser)),
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }
        if let Some((tid, c_type)) = typ {
            Ok(Parameter {
                name: name.into(),
                typ: tid,
                c_type: c_type.unwrap(),
                instance_parameter: instance_parameter,
                direction: direction,
                transfer: transfer,
                nullable: nullable,
                allow_none: allow_none,
            })
        }
        else if varargs {
            Ok(Parameter {
                name: "".into(),
                typ: self.find_type(INTERNAL_NAMESPACE, "varargs").unwrap(),
                c_type: "".into(),
                instance_parameter: instance_parameter,
                direction: Default::default(),
                transfer: Transfer::None,
                nullable: nullable,
                allow_none: allow_none,
            })
        }
        else {
            Err(mk_error!("Missing <type> element", parser))
        }
    }

    fn read_type(&mut self, parser: &mut Reader, ns_id: u16,
                 name: &OwnedName, attrs: &Attributes) -> Result<(TypeId, Option<String>), Error> {
        let start_pos = parser.position();
        let name = try!(attrs.get("name")
                        .or_else(|| if name.local_name == "array" { Some("array") } else { None })
                        .ok_or_else(|| mk_error!("Missing type name", &start_pos)));
        let c_type = attrs.get("type").map(|s| s.into());
        let mut inner = Vec::new();
        loop {
            let event = parser.next();
            match event {
                StartElement { name, attributes, .. } => {
                    match name.local_name.as_ref() {
                        "type" | "array" => {
                            inner.push(try!(self.read_type(parser, ns_id, &name, &attributes)).0);
                        }
                        x => return Err(mk_error!(format!("Unexpected element <{}>", x), parser)),
                    }
                }
                EndElement { .. } => break,
                _ => xml_try!(event, parser),
            }
        }
        if inner.is_empty() || name == "GLib.ByteArray" {
            if name == "array" {
                Err(mk_error!("Missing element type", &start_pos))
            }
            else {
                Ok((self.find_or_stub_type(ns_id, name), c_type))
            }
        }
        else {
            let tid = if name == "array" {
                Type::c_array(self, inner[0], attrs.get("fixed-size").and_then(|n| n.parse().ok()))
            }
            else {
                try!(Type::container(self, name, inner)
                               .ok_or_else(|| mk_error!("Unknown container type", &start_pos)))
            };
            Ok((tid, c_type))
        }
    }
}

trait Get {
    fn get<'a>(&'a self, name: &str) -> Option<&'a str>;
}

impl Get for Attributes {
    fn get<'a>(&'a self, name: &str) -> Option<&'a str> {
        for attr in self {
            if attr.name.local_name == name {
                return Some(&attr.value);
            }
        }
        None
    }
}

fn ignore_element(parser: &mut Reader) -> Result<(), Error> {
    loop {
        let event = parser.next();
        match event {
            StartElement { .. } => try!(ignore_element(parser)),
            EndElement { .. } => return Ok(()),
            _ => xml_try!(event, parser),
        }
    }
}

fn make_file_name(dir: &str, name: &str) -> PathBuf {
    let mut path = PathBuf::from(dir);
    let name = format!("{}.gir", name);
    path.push(name);
    path
}

fn to_bool(s: &str) -> bool {
    s == "1"
}
