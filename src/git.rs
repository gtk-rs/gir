use std::path::Path;
use std::process::Command;

pub fn repo_hash<P: AsRef<Path>>(path: P) -> Option<String> {
    let git_path = path.as_ref().to_str();
    let mut args = match git_path {
        Some(path) => vec!["-C", path],
        None => vec![],
    };
    args.extend(&["rev-parse", "--short", "HEAD"]);
    let hash = String::from_utf8(Command::new("git").args(&args).output().ok()?.stdout).ok()?;
    let hash = hash.trim_end_matches('\n');

    if dirty(path) {
        Some(format!("{}+", hash))
    } else {
        Some(hash.into())
    }
}

fn dirty<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref().to_str();
    let mut args = match path {
        Some(path) => vec!["-C", path],
        None => vec![],
    };
    args.extend(&["ls-files", "-m"]);
    match Command::new("git").args(&args).output() {
        Ok(modified_files) => !modified_files.stdout.is_empty(),
        Err(_) => false,
    }
}
