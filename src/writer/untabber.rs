use std::io::{Result, Write};

use super::TAB;

pub struct Untabber {
    orig: Box<dyn Write>,
}

impl Untabber {
    pub fn new(orig: Box<dyn Write>) -> Self {
        Self { orig }
    }
}

impl Write for Untabber {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut chunks = buf.split(|b| b == &b'\t').peekable();
        while let Some(chunk) = chunks.next() {
            self.orig.write_all(chunk)?;
            if chunks.peek().is_some() {
                self.orig.write_all(TAB.as_bytes())?;
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
