use std::vec::Vec;

pub enum Chunk {
    Comment(Vec<Chunk>),
    BlockHalf(Vec<Chunk>), //Block without open bracket, temporary
    UnsafeSmart(Vec<Chunk>),  //TODO: remove (will change generated results)
    Unsafe(Vec<Chunk>),
    FfiCallTODO(String),
    FfiCall{name: String, prefix: String, suffix: String, params: Vec<Chunk>},
    FfiCallParameter(String),
}

pub fn chunks(ch: Chunk) -> Vec<Chunk> {
    vec![ch]
}
