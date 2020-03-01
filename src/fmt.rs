use log::warn;
use std::path::Path;
use std::process::Command;

const RUSTFMT: &str = "rustfmt";

pub fn check_rustfmt() -> bool {
    let output = Command::new(RUSTFMT).arg("--version").output();
    output.is_ok()
}

pub fn format(path: &Path) {
    let output = Command::new(RUSTFMT).arg("-q").arg(path).output();
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
        Err(_) => { /*We checked rustfmt presence in check_rustfmt, so can ignore errors*/ }
    }
}
