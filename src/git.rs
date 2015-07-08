use std::path::Path;
use git2::{Repository, StatusOptions};

pub fn repo_hash<P :AsRef<Path>>(path: P) -> Result<String, ()> {
    if let Ok(repo) = Repository::open(path) {
        if let Ok(buf) = repo.revparse_single("HEAD").and_then(|obj| obj.short_id()) {
            if let Some(s) = buf.as_str() {
                if dirty(&repo) {
                    return Ok(format!("{}+", s))
                }
                else {
                    return Ok(s.into());
                }
            }
        }
    }
    Err(())
}

fn dirty(repo: &Repository) -> bool {
    repo.statuses(
        Some(StatusOptions::new().include_ignored(false).include_untracked(false)
             .include_unmodified(false)))
        .ok().map(|s| s.len() != 0).unwrap_or(false)
}
