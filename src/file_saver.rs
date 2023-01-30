use std::{
    fs::{self, File},
    io::{BufWriter, Result, Write},
    path::Path,
};

use crate::writer::untabber::Untabber;

pub fn save_to_file<P, F>(path: P, make_backup: bool, mut closure: F)
where
    P: AsRef<Path>,
    F: FnMut(&mut dyn Write) -> Result<()>,
{
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    if make_backup {
        let _backuped = create_backup(path)
            .unwrap_or_else(|why| panic!("couldn't create backup for {path:?}: {why:?}"));
    }
    let file = File::create(path).unwrap_or_else(|why| panic!("couldn't create {path:?}: {why}"));
    let writer = BufWriter::new(file);
    let mut untabber = Untabber::new(Box::new(writer));
    closure(&mut untabber).unwrap_or_else(|why| panic!("couldn't write to {path:?}: {why:?}"));
}

/// Create .bak file
pub fn create_backup<P: AsRef<Path>>(path: P) -> Result<bool> {
    if fs::metadata(&path).is_err() {
        return Ok(false);
    }
    let new_path = path.as_ref().with_extension("bak");
    fs::rename(path, new_path).map(|_| true)
}
