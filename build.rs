use std::{fs::File, io::Write};

#[path = "src/git.rs"]
mod git;

fn main() {
    let ver = git::repo_hash(".").unwrap_or_else(|| "???".into());

    File::create("src/gir_version.rs")
        .and_then(|mut f| writeln!(f, "pub const VERSION: &str = \"{}\";", ver))
        .unwrap();
}
