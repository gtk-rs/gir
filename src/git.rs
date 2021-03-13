use std::io::Result;
use std::path::Path;
use std::process::{Command, Output};

fn git_command(path: impl AsRef<Path>, subcommand: &[&str]) -> Result<Output> {
    let git_path = path
        .as_ref()
        .to_str()
        .expect("Repository path must be a valid UTF-8 string");

    let mut args = vec!["-C", git_path];
    args.extend(subcommand);

    Command::new("git").args(&args).output()
}

pub fn repo_hash(path: impl AsRef<Path>) -> Option<String> {
    let output = git_command(path.as_ref(), &["rev-parse", "--short", "HEAD"]).ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8(output.stdout).ok()?;
    let hash = hash.trim_end_matches('\n');

    if dirty(path) {
        Some(format!("{}+", hash))
    } else {
        Some(hash.into())
    }
}

fn dirty(path: impl AsRef<Path>) -> bool {
    match git_command(path.as_ref(), &["ls-files", "-m"]) {
        Ok(modified_files) => !modified_files.stdout.is_empty(),
        Err(_) => false,
    }
}
