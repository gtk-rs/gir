use std::vec::Vec;

pub enum Chunk {
    Comment(Vec<Chunk>),
    BlockHalf(Vec<Chunk>), //Block without open bracket, temporary
    Unsafe(Vec<Chunk>),
    FfiCallTODO(String),
}

pub fn chunks(ch: Chunk) -> Vec<Chunk> {
    vec![ch]
}
