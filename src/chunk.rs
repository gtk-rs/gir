use std::iter::*;
use std::fmt::Debug;

/// Chunk of code text
#[derive(Debug)]
pub enum Chunk {
    Text(String),
}

impl ToString for Chunk {
    #[inline]
    fn to_string(&self) -> String {
        match self {
            &Chunk::Text(ref str) => str.clone(),
        }
    }
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

// for slice 'str
impl<'a, T: IntoChunk> IntoChunk for &'a T {
    #[inline]
    fn into_chunk(&self) -> Chunk {
        (*self).into_chunk()
    }
}

#[derive(Clone)]
pub struct IntoChunkIter<I> {
    iter: I
}

impl<I: Iterator> Iterator for IntoChunkIter<I> where
    I::Item: IntoChunk {
    type Item = Chunk;

    #[inline]
    fn next(&mut self) -> Option<Chunk> {
        self.iter.next().map(|a| a.into_chunk())
    }
}

pub trait IntoChunkIterator {
    fn into_chunk_iter(self) -> IntoChunkIter<Self> where Self: Sized {
        IntoChunkIter{iter: self}
    }
}

impl<T, I: IntoIterator<Item=T>> IntoChunkIterator for I { }

//For debug prints
//Use: let tmp: IteratorPrinter = iter.collect();
pub struct IteratorPrinter;

impl<T: Debug> FromIterator<T> for IteratorPrinter {
    fn from_iter<I: IntoIterator<Item=T>>(iterable: I) -> IteratorPrinter {
        for i in iterable { println!("{:?}", i); }
        IteratorPrinter
    }
}
