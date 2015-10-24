use std::io::{Result, Write};
use super::TAB;

pub struct Untabber {
    orig: Box<Write>,
}

impl Untabber {
    pub fn new(orig: Box<Write>) -> Untabber {
        Untabber{ orig: orig }
    }
}

impl Write for Untabber {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut chunks = buf.split(|b| b == &b'\t').peekable();
        loop {
            match chunks.next() {
                Some(chunk) => try!(self.orig.write_all(chunk)),
                None => break,
            };
            if chunks.peek().is_some() {
                try!(self.orig.write_all(TAB.as_bytes()));
            } else {
                break;
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> Result<()> {
        self.orig.flush()
    }
}
