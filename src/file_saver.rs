use std::fs;
use std::fs::File;
use std::io::Result;
use std::path::Path;

pub fn save_to_file<P, F>(path: P, closure: &mut F) where
    P: AsRef<Path>, F: FnMut(&mut File) -> Result<()> {
    let _backuped = create_backup(&path)
        .unwrap_or_else(|why| panic!("couldn't create backup for {:?}: {:?}", path.as_ref(), why));
    let mut file = File::create(&path)
        .unwrap_or_else(|why| panic!("couldn't create {:?}: {}", path.as_ref(), why));
    closure(&mut file)
        .unwrap_or_else(|why| panic!("couldn't write to {:?}: {:?}", path.as_ref(), why));
}

/// Create .bak file
pub fn create_backup<P: AsRef<Path>>(path: P) -> Result<bool> {
    match fs::metadata(&path) {
        Err(_) => return Ok(false),
        Ok(_) => (),
    }
    let new_path = path.as_ref().with_extension("bak");
    fs::rename(path, new_path).map(|_| true)
}
