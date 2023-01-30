use std::{
    fmt,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    rc::Rc,
    str,
};

use xml::{
    self,
    attribute::OwnedAttribute,
    common::{Position, TextPosition},
    name::OwnedName,
    reader::{EventReader, XmlEvent},
};

/// NOTE: After parser returns an error its further behaviour is unspecified.
pub struct XmlParser<'a> {
    /// Inner XML parser doing actual work.
    parser: EventReader<Box<dyn 'a + Read>>,
    /// Next event to be returned.
    ///
    /// Takes priority over events returned from inner parser.
    /// Used to support peaking one element ahead.
    peek_event: Option<Result<XmlEvent, String>>,
    /// Position on peek event if any.
    peek_position: TextPosition,
    /// Used to emits errors. Rc so that it can be cheaply shared with Element
    /// type.
    error_emitter: Rc<ErrorEmitter>,
}

struct ErrorEmitter {
    /// Path to currently parsed document.
    path: Option<PathBuf>,
}

impl ErrorEmitter {
    pub fn emit(&self, message: &str, position: TextPosition) -> String {
        let enriched = match self.path {
            Some(ref path) => format!("{} at line {}: {}", path.display(), position, message),
            None => format!("{position} {message}"),
        };
        format!("GirXml: {enriched}")
    }

    pub fn emit_error(&self, error: &xml::reader::Error) -> String {
        // Error returned by EventReader already includes the position.
        // That is why we have a separate implementation that only
        // prepends the file path.
        let enriched = match self.path {
            Some(ref path) => format!("{}:{}", path.display(), error),
            None => format!("{error}"),
        };
        format!("GirXml: {enriched}")
    }
}

/// A wrapper for `XmlEvent::StartDocument` which doesn't have its own type.
pub struct Document;

/// A wrapper for `XmlEvent::StartElement` which doesn't have its own type.
pub struct Element {
    name: OwnedName,
    attributes: Vec<OwnedAttribute>,
    position: TextPosition,
    error_emitter: Rc<ErrorEmitter>,
}

impl Element {
    /// Returns the element local name.
    pub fn name(&self) -> &str {
        &self.name.local_name
    }

    /// Value of attribute with given name or None if it is not found.
    pub fn attr(&self, name: &str) -> Option<&str> {
        for attr in &self.attributes {
            if attr.name.local_name == name {
                return Some(&attr.value);
            }
        }
        None
    }

    /// Checks if elements has any attributes.
    pub fn has_attrs(&self) -> bool {
        !self.attributes.is_empty()
    }

    pub fn attr_bool(&self, name: &str, default: bool) -> bool {
        for attr in &self.attributes {
            if attr.name.local_name == name {
                return attr.value == "1";
            }
        }
        default
    }

    pub fn attr_from_str<T>(&self, name: &str) -> Result<Option<T>, String>
    where
        T: str::FromStr,
        T::Err: fmt::Display,
    {
        if let Some(value_str) = self.attr(name) {
            match T::from_str(value_str) {
                Ok(value) => Ok(Some(value)),
                Err(error) => {
                    let message = format!(
                        "Attribute `{}` on element <{}> has invalid value: {}",
                        name,
                        self.name(),
                        error
                    );
                    Err(self.error_emitter.emit(&message, self.position))
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Returns element position.
    pub fn position(&self) -> TextPosition {
        self.position
    }

    /// Value of attribute with given name or an error when absent.
    pub fn attr_required(&self, name: &str) -> Result<&str, String> {
        for attr in &self.attributes {
            if attr.name.local_name == name {
                return Ok(&attr.value);
            }
        }
        let message = format!(
            "Attribute `{}` on element <{}> is required.",
            name,
            self.name()
        );
        Err(self.error_emitter.emit(&message, self.position))
    }
}

impl<'a> XmlParser<'a> {
    pub fn from_path(path: &Path) -> Result<XmlParser<'_>, String> {
        match File::open(path) {
            Err(e) => Err(format!("Can't open file \"{}\": {}", path.display(), e)),
            Ok(file) => Ok(XmlParser {
                parser: EventReader::new(Box::new(BufReader::new(file))),
                peek_event: None,
                peek_position: TextPosition::new(),
                error_emitter: Rc::new(ErrorEmitter {
                    path: Some(path.to_owned()),
                }),
            }),
        }
    }

    #[cfg(test)]
    pub fn new<'r, R: 'r + Read>(read: R) -> XmlParser<'r> {
        XmlParser {
            parser: EventReader::new(Box::new(read)),
            peek_event: None,
            peek_position: TextPosition::new(),
            error_emitter: Rc::new(ErrorEmitter { path: None }),
        }
    }

    /// Returns an error that combines current position and given error message.
    pub fn fail(&self, message: &str) -> String {
        self.error_emitter.emit(message, self.position())
    }

    /// Returns an error that combines given error message and position.
    pub fn fail_with_position(&self, message: &str, position: TextPosition) -> String {
        self.error_emitter.emit(message, position)
    }

    pub fn unexpected_element(&self, elem: &Element) -> String {
        let message = format!("Unexpected element <{}>", elem.name());
        self.error_emitter.emit(&message, elem.position())
    }

    fn unexpected_event(&self, event: &XmlEvent) -> String {
        let message = format!("Unexpected event {event:?}");
        self.error_emitter.emit(&message, self.position())
    }

    pub fn position(&self) -> TextPosition {
        match self.peek_event {
            None => self.parser.position(),
            Some(_) => self.peek_position,
        }
    }

    /// Returns next XML event without consuming it.
    fn peek_event(&mut self) -> &Result<XmlEvent, String> {
        if self.peek_event.is_none() {
            self.peek_event = Some(self.next_event_impl());
            self.peek_position = self.parser.position();
        }
        self.peek_event.as_ref().unwrap()
    }

    /// Consumes and returns next XML event.
    fn next_event(&mut self) -> Result<XmlEvent, String> {
        match self.peek_event.take() {
            None => self.next_event_impl(),
            Some(e) => e,
        }
    }

    /// Returns next XML event directly from parser.
    fn next_event_impl(&mut self) -> Result<XmlEvent, String> {
        loop {
            match self.parser.next() {
                // Ignore whitespace and comments by default.
                Ok(XmlEvent::Whitespace(..) | XmlEvent::Comment(..)) => continue,
                Ok(event) => return Ok(event),
                Err(e) => return Err(self.error_emitter.emit_error(&e)),
            }
        }
    }

    pub fn document<R, F>(&mut self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut XmlParser<'_>, Document) -> Result<R, String>,
    {
        let doc = self.start_document()?;
        let result = f(self, doc)?;
        self.end_document()?;
        Ok(result)
    }

    fn start_document(&mut self) -> Result<Document, String> {
        match self.next_event()? {
            XmlEvent::StartDocument { .. } => Ok(Document),
            e => Err(self.unexpected_event(&e)),
        }
    }

    fn end_document(&mut self) -> Result<(), String> {
        match self.next_event()? {
            XmlEvent::EndDocument { .. } => Ok(()),
            e => Err(self.unexpected_event(&e)),
        }
    }

    pub fn elements<R, F>(&mut self, mut f: F) -> Result<Vec<R>, String>
    where
        F: FnMut(&mut XmlParser<'_>, &Element) -> Result<R, String>,
    {
        let mut results = Vec::new();
        loop {
            match *self.peek_event() {
                Ok(XmlEvent::StartElement { .. }) => {
                    let element = self.start_element()?;
                    results.push(f(self, &element)?);
                    self.end_element()?;
                }
                _ => return Ok(results),
            }
        }
    }

    pub fn element_with_name<R, F>(&mut self, expected_name: &str, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut XmlParser<'_>, &Element) -> Result<R, String>,
    {
        let elem = self.start_element()?;
        if expected_name != elem.name.local_name {
            return Err(self.unexpected_element(&elem));
        }
        let result = f(self, &elem)?;
        self.end_element()?;
        Ok(result)
    }

    fn start_element(&mut self) -> Result<Element, String> {
        match self.next_event() {
            Ok(XmlEvent::StartElement {
                name, attributes, ..
            }) => Ok(Element {
                name,
                attributes,
                position: self.position(),
                error_emitter: self.error_emitter.clone(),
            }),
            Ok(e) => Err(self.unexpected_event(&e)),
            Err(e) => Err(e),
        }
    }

    fn end_element(&mut self) -> Result<(), String> {
        match self.next_event() {
            Ok(XmlEvent::EndElement { .. }) => Ok(()),
            Ok(e) => Err(self.unexpected_event(&e)),
            Err(e) => Err(e),
        }
    }

    pub fn text(&mut self) -> Result<String, String> {
        let mut result = String::new();
        loop {
            match *self.peek_event() {
                Ok(XmlEvent::Characters(..)) => {
                    if let Ok(XmlEvent::Characters(s)) = self.next_event() {
                        result.push_str(&s);
                    }
                }
                Err(_) => {
                    self.next_event()?;
                    unreachable!();
                }
                _ if result.is_empty() => {
                    return Err(self.fail("Expected text content"));
                }
                _ => break,
            }
        }
        Ok(result)
    }

    /// Ignore everything within current element.
    pub fn ignore_element(&mut self) -> Result<(), String> {
        let mut depth = 1;
        loop {
            match *self.peek_event() {
                Ok(XmlEvent::StartElement { .. }) => {
                    // Ignore warning about unused result, we know event is OK.
                    drop(self.next_event());
                    depth += 1;
                }
                Ok(XmlEvent::EndElement { .. }) => {
                    depth -= 1;
                    if depth > 0 {
                        drop(self.next_event());
                    } else {
                        return Ok(());
                    }
                }
                Ok(_) => drop(self.next_event()),
                Err(_) => return self.next_event().map(|_| ()),
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn with_parser<F, R>(xml: &[u8], f: F) -> Result<R, String>
    where
        F: FnOnce(XmlParser<'_>) -> Result<R, String>,
    {
        f(XmlParser::new(xml))
    }

    #[test]
    fn test_element_with_name() {
        fn parse_with_root_name(xml: &[u8], root: &str) -> Result<(), String> {
            with_parser(xml, |mut p| {
                p.document(|p, _| p.element_with_name(root, |_, _elem| Ok(())))
            })
        }

        let xml = br#"<?xml version="1.0"?>
            <!-- a comment -->
            <a>
            </a>"#;

        assert!(parse_with_root_name(xml, "a").is_ok());
        assert!(parse_with_root_name(xml, "b").is_err());
    }

    #[test]
    fn test_ignore_element() {
        let xml = br#"<?xml version="1.0"?>
            <a>
                <b>
                    <c/>
                    <d/>
                </b>
                <b> some text content </b>
            </a>"#;

        with_parser(xml, |mut p| {
            p.document(|p, _| p.element_with_name("a", |p, _| p.ignore_element()))
        })
        .unwrap();
    }

    #[test]
    fn test_elements() {
        let xml = br#"<?xml version="1.0"?>
            <root>
                <child name="a" />
                <child name="b" />
                <child name="c" />
            </root>"#;

        let result: String = with_parser(xml, |mut p| {
            p.document(|p, _| {
                p.element_with_name("root", |p, _| {
                    p.elements(|_, elem| elem.attr_required("name").map(|s| s.to_owned()))
                        .map(|v| v.join("."))
                })
            })
        })
        .unwrap();

        assert_eq!("a.b.c", result);
    }

    #[test]
    fn test_text() {
        let xml = br#"<?xml version="1.0"?>
            <x>hello world!</x>"#;

        let result: String = with_parser(xml, |mut p| {
            p.document(|p, _| p.element_with_name("x", |p, _| p.text()))
        })
        .unwrap();

        assert_eq!("hello world!", &result);
    }

    #[test]
    fn test_attr_required() {
        let xml = br#"<?xml version="1.0"?>
            <x a="1" b="2"></x>"#;

        with_parser(xml, |mut p| {
            p.document(|p, _| {
                p.element_with_name("x", |_, elem| {
                    assert!(elem.attr_required("a").is_ok());
                    assert!(elem.attr_required("b").is_ok());
                    assert!(elem.attr_required("c").is_err());
                    assert!(elem.attr_required("d").is_err());
                    Ok(())
                })
            })
        })
        .unwrap();
    }

    #[test]
    fn test_attr_from_str() {
        let xml = br#"<?xml version="1.0"?>
            <x a="123" b="2a"></x>"#;

        with_parser(xml, |mut p| {
            p.document(|p, _| {
                p.element_with_name("x", |_, elem| {
                    assert_eq!(elem.attr_from_str::<usize>("a").unwrap(), Some(123));
                    assert!(elem.attr_from_str::<usize>("b").is_err());
                    Ok(())
                })
            })
        })
        .unwrap();
    }
}
