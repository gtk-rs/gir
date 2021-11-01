// use std::{
//     fmt,
//     fs::File,
//     io::{BufReader, Read},
//     path::{Path, PathBuf},
//     rc::Rc,
//     str,
// };
// use xml::{
//     self,
//     attribute::OwnedAttribute,
//     common::{Position, TextPosition},
//     name::OwnedName,
//     reader::{EventReader, XmlEvent},
// };

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::str::FromStr;

pub struct XmlParser {
    reader: Reader<BufReader<File>>,
    file_path: PathBuf,
}

impl XmlParser {
    pub fn new(file_path: PathBuf) -> Result<Self, String> {
        let f = File::open(&file_path)
            .map_err(|e| format!("Failed to open `{}`: {}", file_path.display(), e))?;
        let f = BufReader::new(f);

        Ok(Self {
            reader: Reader::from_reader(f),
            file_path,
        })
    }

    pub fn get_next_if_tag_is<'a, 'b: 'a>(
        &mut self,
        buf: &'b mut Vec<u8>,
        expected_tag: &[u8],
    ) -> Result<BytesStart<'a>, String> {
        match self.next_event(buf)? {
            Event::Start(e) => {
                if e.name() == expected_tag {
                    Ok(e)
                } else {
                    Err(format!("Unexpected element <{}>", unsafe {
                        std::str::from_utf8_unchecked(e.name())
                    },))
                }
            }
            Event::Text(e) => Err("Expected a tag, found text".to_owned()),
            _ => unreachable!(),
        }
    }

    pub fn next_event<'a, 'b: 'a>(&mut self, buf: &'b mut Vec<u8>) -> Result<Event<'a>, String> {
        self.reader.read_event(buf).map_err(|e| {
            format!(
                "Error at position {}: {:?}",
                self.reader.buffer_position(),
                e
            )
        })
    }

    pub fn next_element<'a, 'b: 'a>(&mut self, buf: &'b mut Vec<u8>) -> Result<BytesStart<'a>, String> {
        loop {
            return match self.next_event(buf) {
                Ok(Event::Start(e)) => Ok(e),
                Ok(Event::End(_)) => Err("Unexpected Event::End".to_owned()),
                Ok(Event::Eof) => Err("Reached end of file".to_owned()),
                Err(e) => Err(format!(
                    "Error at position {}: {:?}",
                    self.reader.buffer_position(),
                    e
                )),
                _ => continue,
            };
        }
    }

    pub fn end_element<'a>(&mut self, buf: &'a mut Vec<u8>) -> Result<(), String> {
        loop {
            return match self.next_event(buf) {
                Ok(Event::End(e)) => Ok(()),
                Ok(Event::Start(_)) => Err("Unexpected Event::Start".to_owned()),
                Ok(Event::Eof) => Err("Reached end of file".to_owned()),
                Err(e) => Err(format!(
                    "Error at position {}: {:?}",
                    self.reader.buffer_position(),
                    e
                )),
                _ => continue,
            };
        }
    }

    pub fn unexpected_element(&self, elem: &BytesStart<'_>) -> String {
        self.error_with_position(&format!(
            "Unexpected element <{}>",
            std::str::from_utf8(elem.name()).unwrap()
        ))
    }

    pub fn error(&self, msg: &str) -> String {
        format!("GirXml {}: {}", self.file_path.display(), msg)
    }

    pub fn error_with_position(&self, msg: &str) -> String {
        format!(
            "GirXml {} at position {}: {}",
            self.file_path.display(),
            self.reader.buffer_position(),
            msg
        )
    }

    /// Ignore everything within current element.
    pub fn ignore_element<'a>(&mut self, buf: &'a mut Vec<u8>) -> Result<(), String> {
        let mut depth = 1;
        loop {
            match self.next_event(buf) {
                Ok(Event::Start(_)) => {
                    // Ignore warning about unused result, we know event is OK.
                    depth += 1;
                }
                Ok(Event::End(_)) => {
                    depth -= 1;
                    if depth < 1 {
                        return Ok(());
                    }
                }
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }
    }

    pub fn text(&mut self, buf: &mut Vec<u8>, elem_name: &[u8]) -> Result<String, String> {
        self.reader.read_text(elem_name, buf).map_err(|e| {
            format!(
                "Error at position {}: {:?}",
                self.reader.buffer_position(),
                e
            )
        })
    }

    pub fn attr_from_str<T>(
        &self,
        elem: &BytesStart<'_>,
        attr_name: &[u8],
    ) -> Result<Option<T>, String>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        if let Some(attr) = elem
            .attributes()
            .filter_map(|n| n.ok())
            .find(|n| n.key == attr_name)
        {
            let value_str = std::str::from_utf8(&attr.value).unwrap();
            match T::from_str(value_str) {
                Ok(value) => Ok(Some(value)),
                Err(error) => {
                    let message = format!(
                        "Attribute `{}` on element <{}> has invalid value: {}",
                        std::str::from_utf8(attr_name).unwrap(),
                        std::str::from_utf8(elem.name()).unwrap(),
                        error,
                    );
                    Err(self.error_with_position(&message))
                }
            }
        } else {
            Ok(None)
        }
    }
}
