extern crate git2;

#[path = "src/git.rs"]
mod git;

use std::fs::File;
use std::io::Write;

fn main() {
    let ver = git::repo_hash(".").unwrap_or_else(|_| "???".into());

    File::create("src/version.rs")
        .and_then(|mut f| writeln!(f, "pub const VERSION: &'static str = \"{}\";", ver))
        .unwrap();
}
