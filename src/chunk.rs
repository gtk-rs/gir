use std::iter::*;
use std::fmt::Debug;

/// Chunk of code text
#[derive(Debug)]
pub enum Chunk {
    Text(String),
    EndLine,
}

impl ToString for Chunk {
    #[inline]
    fn to_string(&self) -> String {
        match self {
            &Chunk::Text(ref str) => str.clone(),
            &Chunk::EndLine => end_line_string(),
        }
    }
}

#[inline]
#[cfg(unix)]
fn end_line_string() -> String {
    "\n".into()
}
#[inline]
#[cfg(windows)]
fn end_line_string() -> String {
    "\r\n".into()
}

pub trait IntoChunk {
    fn into_chunk(&self) -> Chunk;
}

impl IntoChunk for String {
    #[inline]
    fn into_chunk(&self) -> Chunk {
        Chunk::Text(self.clone())
    }
}

impl<'a> IntoChunk for &'a str {
    #[inline]
    fn into_chunk(&self) -> Chunk {
        Chunk::Text(self.to_string())
    }
}

pub trait IntoChunks {
    fn into_chunks(&self) -> Vec<Chunk>;
}

impl IntoChunks for String {
    #[inline]
    fn into_chunks<'a>(&self) -> Vec<Chunk> {
        vec![Chunk::Text(self.clone()), Chunk::EndLine]
    }
}

impl<'a> IntoChunks for &'a str {
    #[inline]
    fn into_chunks(&self) -> Vec<Chunk> {
        vec![Chunk::Text(self.to_string()), Chunk::EndLine]
    }
}

impl IntoChunks for Vec<String> {
    fn into_chunks(&self) -> Vec<Chunk> {
        let mut vec: Vec<Chunk> = Vec::new();
        for s in self {
            let inner_vec = s.into_chunks();
            vec.reserve(inner_vec.len());
            for c in inner_vec {
                vec.push(c);
            }
        }
        vec
    }
}

impl<'a> IntoChunks for Vec<&'a str> {
    fn into_chunks(&self) -> Vec<Chunk> {
        let mut vec: Vec<Chunk> = Vec::new();
        for s in self {
            let inner_vec = s.into_chunks();
            vec.reserve(inner_vec.len());
            for c in inner_vec {
                vec.push(c);
            }
        }
        vec
    }
}

//For debug prints
//Use: let tmp: IteratorPrinter = iter.collect();
pub struct IteratorPrinter;

impl<T: Debug> FromIterator<T> for IteratorPrinter {
    fn from_iter<I: IntoIterator<Item=T>>(iterable: I) -> IteratorPrinter {
        for i in iterable { println!("{:?}", i); }
        IteratorPrinter
    }
}
