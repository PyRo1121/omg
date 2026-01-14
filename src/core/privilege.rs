//! Privilege elevation utilities
//!
//! Automatically elevates to root when needed for system operations,
//! similar to how paru/yay handle this seamlessly.

use std::env;
use std::os::unix::process::CommandExt;
use std::process::Command;

/// Check if we're running as root
#[must_use]
pub fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Re-execute the current command with sudo if not root
/// This replaces the current process - it doesn't return on success
pub fn elevate_if_needed() -> std::io::Result<()> {
    if is_root() {
        return Ok(());
    }

    let args: Vec<String> = env::args().collect();
    let exe = env::current_exe()?;

    // Use sudo to re-execute ourselves
    let err = Command::new("sudo")
        .arg("--")
        .arg(&exe)
        .args(&args[1..])
        .exec();

    // exec() only returns if it failed
    Err(err)
}

/// Execute a closure that requires root, elevating if needed
/// Returns Ok(true) if we elevated (caller should exit), Ok(false) if already root
pub fn with_root<F, T>(f: F) -> anyhow::Result<T>
where
    F: FnOnce() -> anyhow::Result<T>,
{
    if !is_root() {
        // Re-exec with sudo - this replaces the process
        elevate_if_needed().map_err(|e| anyhow::anyhow!("Failed to elevate privileges: {e}"))?;
        // This line is never reached
        unreachable!()
    }
    f()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_root() {
        // In normal test environment, we're not root
        // This test just ensures the function doesn't panic
        let _ = is_root();
    }
}
