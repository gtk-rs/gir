use std::error::Error;
use std::fs;
use std::fs::File;
use std::io::{Result, Write};
use std::path::Path;

use chunk::*;

pub trait SaveToFile {
    fn save_to_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()>;
}

impl<I: Iterator<Item=Chunk>> SaveToFile for I {
    fn save_to_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        match create_backup(&path) {
            Err(why) => panic!("couldn't create backup for {:?}: {}", path.as_ref(),
                            Error::description(&why)),
            Ok(_) => (),
        }
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

/// Create .bak file, only if one not present
pub fn create_backup<P: AsRef<Path>>(path: P) -> Result<bool> {
    match fs::metadata(&path) {
        Err(_) => return Ok(false),
        Ok(_) => (),
    }
    let new_path = path.as_ref().with_extension("bak");
    match fs::metadata(&new_path) {
        Err(_) => (),
        Ok(_) => return Ok(false),
    }
    fs::rename(path, new_path).map(|_| true)
}
