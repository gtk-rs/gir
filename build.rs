use std::{fs::File, io::Write};

// Build.rs does not use all provided functions
#[allow(dead_code)]
#[path = "src/git.rs"]
mod git;

fn main() {
    let repo_path = git::git_dir(".").unwrap();
    println!(
        "cargo:rerun-if-changed={}",
        repo_path.join("HEAD").display()
    );
    let ver = git::repo_hash(".").unwrap_or_else(|| "???".into());

    File::create("src/gir_version.rs")
        .and_then(|mut f| writeln!(f, "pub const VERSION: &str = \"{}\";", ver))
        .unwrap();
}
