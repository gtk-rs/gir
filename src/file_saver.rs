use std::error::Error;
use std::fs::File;
use std::io::{Result, Write};
use std::path::Path;

use chunk::*;

pub trait SaveToFile {
    fn save_to_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()>;
}

impl<I: Iterator<Item=Chunk>> SaveToFile for I {
    fn save_to_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        //TODO: add backup
        let mut file = match File::create(&path) {
            Err(why) => panic!("couldn't create {:?}: {}", path.as_ref(),
                            Error::description(&why)),
            Ok(file) => file,
        };
        for ch in self {
            match file.write_all(&ch.to_string().into_bytes()) {
                Err(why) => panic!("couldn't write to {:?}: {}", path.as_ref(),
                                Error::description(&why)),
                Ok(_) => (),
            }
        }

        Ok(())
    }
}
