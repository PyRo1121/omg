//! Privilege elevation utilities
//!
//! Automatically elevates to root when needed for system operations,
//! similar to how paru/yay handle this seamlessly.

#[cfg(not(test))]
use std::env;
#[cfg(not(test))]
use std::os::unix::process::CommandExt;
#[cfg(not(test))]
use std::process::Command;

/// Check if we're running as root
#[must_use]
pub fn is_root() -> bool {
    rustix::process::geteuid().is_root()
}

/// Re-execute the current command with sudo if not root
/// This replaces the current process - it doesn't return on success
pub fn elevate_if_needed(args: &[String]) -> std::io::Result<()> {
    if is_root() {
        return Ok(());
    }

    #[cfg(test)]
    {
        let _ = args;
        Ok(())
    }

    #[cfg(not(test))]
    {
        let exe = env::current_exe()?;

        // Use sudo to re-execute ourselves
        let err = Command::new("sudo")
            .arg("--")
            .arg(&exe)
            .args(args.get(1..).unwrap_or_default())
            .exec();

        // exec() only returns if it failed
        Err(err)
    }
}

/// Request elevation for a specific operation, checking against a whitelist
pub fn elevate_for_operation(operation: &str, args: &[String]) -> std::io::Result<()> {
    // Security: Only allow elevation for known safe operations
    const ALLOWED_ROOT_OPS: &[&str] = &["install", "remove", "upgrade", "update", "sync", "clean"];

    if !ALLOWED_ROOT_OPS.contains(&operation) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("Operation '{operation}' is not whitelisted for root privileges"),
        ));
    }

    elevate_if_needed(args)
}

/// Run the current executable with sudo and specific arguments asynchronously
pub async fn run_self_sudo(args: &[&str]) -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;

    // Detect if we're running in development/test mode
    let is_dev_build = cfg!(debug_assertions)
        || std::env::var("OMG_TEST_MODE").is_ok()
        || std::env::var("CARGO_PRIMARY_PACKAGE").is_ok();

    if is_dev_build {
        anyhow::bail!(
            "Privilege elevation is not supported in development builds.\n\
             \n\
             For development, either:\n\
             1. Run with appropriate sudo permissions: sudo {} {:?}\n\
             2. Install the release binary: cargo install --path .\n\
             3. Build and install release binary: cargo build --release && sudo cp target/release/omg /usr/local/bin/\n\
             \n\
             In production, this command will automatically elevate when needed.",
            exe.display(),
            args
        );
    }

    // Try non-interactive sudo first (-n flag)
    let status = tokio::process::Command::new("sudo")
        .arg("-n")
        .arg("--")
        .arg(&exe)
        .args(args)
        .status()
        .await;

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => anyhow::bail!("Elevated command failed with exit code: {s}"),
        Err(e) => {
            // If -n failed, provide helpful guidance
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                anyhow::bail!(
                    "This operation requires sudo privileges.\n\
                     \n\
                     For automation/CI, configure sudo with NOPASSWD or use:\n\
                     sudo -n {} {:?}\n\
                     \n\
                     For interactive use, run:\n\
                     sudo {} {:?}",
                    exe.display(),
                    args,
                    exe.display(),
                    args
                );
            }
            anyhow::bail!("Failed to elevate privileges: {e}")
        }
    }
}

/// Execute a closure that requires root, elevating if needed
/// Returns Ok(true) if we elevated (caller should exit), Ok(false) if already root
pub fn with_root<F, T>(f: F) -> anyhow::Result<T>
where
    F: FnOnce() -> anyhow::Result<T>,
{
    if !is_root() {
        let args: Vec<String> = std::env::args().collect();
        // Re-exec with sudo - this replaces the process
        elevate_if_needed(&args)
            .map_err(|e| anyhow::anyhow!("Failed to elevate privileges: {e}"))?;
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

    #[test]
    fn test_elevate_for_operation_whitelist() {
        let empty_args = Vec::new();
        // Allowed operations
        assert!(elevate_for_operation("install", &empty_args).is_ok()); // Should try to elevate (mocked or skipped in test env)
        assert!(elevate_for_operation("remove", &empty_args).is_ok());
        assert!(elevate_for_operation("upgrade", &empty_args).is_ok());
        assert!(elevate_for_operation("update", &empty_args).is_ok());
        assert!(elevate_for_operation("sync", &empty_args).is_ok());
        assert!(elevate_for_operation("clean", &empty_args).is_ok());

        // Disallowed operations
        assert!(elevate_for_operation("search", &empty_args).is_err());
        assert!(elevate_for_operation("info", &empty_args).is_err());
        assert!(elevate_for_operation("status", &empty_args).is_err());
        assert!(elevate_for_operation("evil_command", &empty_args).is_err());
        assert!(elevate_for_operation("install; rm -rf /", &empty_args).is_err());
    }

    #[test]
    fn test_elevate_if_needed_behavior() {
        // In a unit test, we can't easily check if it execs sudo without mocking Command
        // but we can verify it returns Ok(()) if we pretend to be root (which we can't easily mock here without restructuring)
        // So we focus on the whitelist logic above which is the critical decision logic.
    }
}
