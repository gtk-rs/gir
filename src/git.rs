use std::ffi::OsString;
use std::io::Result;
use std::os::unix::prelude::OsStringExt;
use std::path::{Path, PathBuf};
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
    let output = git_command(path.as_ref(), &["rev-parse", "--short=12", "HEAD"]).ok()?;
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

fn gitmodules_config(subcommand: &[&str]) -> Option<String> {
    let mut args = vec!["config", "-f", ".gitmodules", "-z"];
    args.extend(subcommand);
    let output = git_command(Path::new("."), &args).ok()?;

    if !output.status.success() {
        return None;
    }

    let mut result = String::from_utf8(output.stdout).ok()?;
    assert_eq!(result.pop(), Some('\0'));
    Some(result)
}

fn path_command(path: impl AsRef<Path>, subcommand: &[&str]) -> Option<PathBuf> {
    let mut output = git_command(path, subcommand).ok()?;

    if !output.status.success() {
        return None;
    }

    assert_eq!(
        output
            .stdout
            .pop()
            .map(|c| c as u32)
            .and_then(std::char::from_u32),
        Some('\n')
    );
    let toplevel = OsString::from_vec(output.stdout);
    Some(toplevel.into())
}

pub fn toplevel(path: impl AsRef<Path>) -> Option<PathBuf> {
    path_command(path, &["rev-parse", "--show-toplevel"])
}

// Only build.rs uses this
#[allow(dead_code)]
pub fn git_dir(path: impl AsRef<Path>) -> Option<PathBuf> {
    path_command(path, &["rev-parse", "--git-dir"])
}

pub(crate) fn repo_remote_url(path: impl AsRef<Path>) -> Option<String> {
    // Find the subsection that defines the module for the given path:
    let key_for_path = gitmodules_config(&[
        "--name-only",
        "--get-regexp",
        r"submodule\..+\.path",
        &format!("^{}$", path.as_ref().display()),
    ])?;

    let subsection = key_for_path
        .strip_suffix(".path")
        .expect("submodule.<subsection>.path should end with '.path'");

    gitmodules_config(&["--get", &format!("{}.url", subsection)])
}
