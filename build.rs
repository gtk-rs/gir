use std::{fs::File, io::Write};

// Build.rs does not use all provided functions
#[allow(dead_code)]
#[path = "src/git.rs"]
mod git;

fn main() {
    let repo_path = git::git_dir(".").unwrap();
    let head_path = repo_path.join("HEAD");
    println!("cargo:rerun-if-changed={}", head_path.display());
    let head = std::fs::read_to_string(&head_path).unwrap();
    if let Some(ref_) = head.trim_end().strip_prefix("ref: ") {
        let ref_path = repo_path.join(ref_);
        assert!(ref_path.is_file());
        println!("cargo:rerun-if-changed={}", ref_path.display());
    }
    let ver = git::repo_hash(".").unwrap_or_else(|| "???".into());

    File::create("src/gir_version.rs")
        .and_then(|mut f| writeln!(f, "pub const VERSION: &str = \"{ver}\";",))
        .unwrap();
}
