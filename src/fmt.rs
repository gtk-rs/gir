use std::{path::Path, process::Command};

use log::warn;

/// Check if `cargo fmt` available
pub fn check_fmt() -> bool {
    let output = Command::new("cargo").arg("fmt").arg("--version").output();
    if let Ok(output) = output {
        output.status.success()
    } else {
        false
    }
}

/// Run `cargo fmt` on path
pub fn format(path: &Path) {
    let output = Command::new("cargo").arg("fmt").current_dir(path).output();
    match output {
        Ok(output) if output.status.success() => {}
        Ok(output) => {
            warn!(
                "Failed to format {}:\n{}\n{}",
                path.display(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Err(_) => { /*We checked `cargo` fmt presence in check_fmt, so can ignore errors*/ }
    }
}
