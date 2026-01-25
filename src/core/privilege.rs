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
use std::sync::atomic::{AtomicBool, Ordering};

/// Global flag to track if --yes was specified for non-interactive mode
static YES_FLAG: AtomicBool = AtomicBool::new(false);

/// Set the yes flag globally (call this at the start of main if --yes is present)
pub fn set_yes_flag(value: bool) {
    YES_FLAG.store(value, Ordering::SeqCst);
}

/// Check if the yes flag is set
pub fn get_yes_flag() -> bool {
    YES_FLAG.load(Ordering::SeqCst)
}

/// Trait for privilege checking and elevation (for dependency injection)
pub trait PrivilegeChecker: Send + Sync {
    /// Check if running as root
    fn is_root(&self) -> bool;

    /// Elevate privileges for the given operation and arguments
    fn elevate(&self, operation: &str, args: &[String]) -> std::io::Result<()>;
}

/// Default privilege checker using real system calls
pub struct SystemPrivilegeChecker;

impl PrivilegeChecker for SystemPrivilegeChecker {
    fn is_root(&self) -> bool {
        rustix::process::geteuid().is_root()
    }

    fn elevate(&self, operation: &str, args: &[String]) -> std::io::Result<()> {
        elevate_for_operation(operation, args)
    }
}

/// Mock privilege checker for testing
#[cfg(test)]
pub struct MockPrivilegeChecker {
    pub is_root_value: bool,
    pub should_elevate: bool,
    pub elevation_log: std::sync::Arc<std::sync::Mutex<Vec<(String, Vec<String>)>>>,
}

#[cfg(test)]
impl Default for MockPrivilegeChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl MockPrivilegeChecker {
    pub fn new() -> Self {
        Self {
            is_root_value: false,
            should_elevate: true,
            elevation_log: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn set_root(&mut self, is_root: bool) {
        self.is_root_value = is_root;
    }

    pub fn set_elevation_allowed(&mut self, allowed: bool) {
        self.should_elevate = allowed;
    }

    pub fn get_elevation_log(&self) -> Vec<(String, Vec<String>)> {
        self.elevation_log.lock().unwrap().clone()
    }
}

#[cfg(test)]
impl PrivilegeChecker for MockPrivilegeChecker {
    fn is_root(&self) -> bool {
        self.is_root_value
    }

    fn elevate(&self, operation: &str, args: &[String]) -> std::io::Result<()> {
        self.elevation_log
            .lock()
            .unwrap()
            .push((operation.to_string(), args.to_vec()));

        if self.should_elevate {
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Mock elevation denied",
            ))
        }
    }
}

/// Global privilege checker (can be swapped in tests)
#[cfg(test)]
static PRIVILEGE_CHECKER: std::sync::OnceLock<std::sync::Arc<dyn PrivilegeChecker>> =
    std::sync::OnceLock::new();

#[cfg(test)]
pub fn set_privilege_checker(checker: std::sync::Arc<dyn PrivilegeChecker>) {
    let _ = PRIVILEGE_CHECKER.set(checker);
}

#[cfg(test)]
pub fn get_privilege_checker() -> std::sync::Arc<dyn PrivilegeChecker> {
    PRIVILEGE_CHECKER
        .get()
        .cloned()
        .unwrap_or_else(|| std::sync::Arc::new(SystemPrivilegeChecker))
}

/// Check if we're running as root
#[must_use]
pub fn is_root() -> bool {
    #[cfg(test)]
    {
        get_privilege_checker().is_root()
    }

    #[cfg(not(test))]
    {
        rustix::process::geteuid().is_root()
    }
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

        // Check if --yes or -y flag is present for non-interactive mode
        let yes_flag = args.iter().any(|a| a == "--yes" || a == "-y");

        let mut cmd = Command::new("sudo");

        // Add -n flag for non-interactive mode when --yes is specified
        if yes_flag {
            cmd.arg("-n");
        }

        let err = cmd
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

    // Check if --yes flag is set for non-interactive mode
    let yes_flag = get_yes_flag();

    // If --yes is specified, use only non-interactive sudo (-n flag)
    // Otherwise, try -n first and fall back to interactive mode
    if yes_flag {
        // Non-interactive mode: use -n flag and fail if password required
        let status = tokio::process::Command::new("sudo")
            .arg("-n")
            .arg("--")
            .arg(&exe)
            .args(args)
            .status()
            .await;

        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => {
                // Command ran but failed with non-zero exit code
                anyhow::bail!("Elevated command failed with exit code: {s}")
            }
            Err(e) => {
                // In non-interactive mode, provide clear error message
                anyhow::bail!(
                    "Failed to elevate privileges in non-interactive mode (--yes flag).\n\
                     \n\
                     Error: {e}\n\
                     \n\
                     For non-interactive sudo, configure NOPASSWD in sudoers:\n\
                     sudo visudo\n\
                     \n\
                     Add line (replace username):\n\
                     username ALL=(ALL) NOPASSWD: {}\n\
                     \n\
                     Or remove --yes flag to allow password prompt.",
                    exe.display()
                )
            }
        }
    } else {
        // Interactive mode: try -n first, fall back to interactive sudo
        let status = tokio::process::Command::new("sudo")
            .arg("-n")
            .arg("--")
            .arg(&exe)
            .args(args)
            .status()
            .await;

        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => {
                // Command ran but failed with non-zero exit code
                anyhow::bail!("Elevated command failed with exit code: {s}")
            }
            Err(e) => {
                // Check if this is a permission denied error from sudo -n
                // (which happens when password is required)
                if e.kind() == std::io::ErrorKind::PermissionDenied
                    || e.to_string().contains("permission denied")
                    || e.to_string().contains("no tty present")
                {
                    // Fall back to interactive sudo (allows password prompt)
                    let interactive_status = tokio::process::Command::new("sudo")
                        .arg("--")
                        .arg(&exe)
                        .args(args)
                        .status()
                        .await;

                    return match interactive_status {
                        Ok(s) if s.success() => Ok(()),
                        Ok(s) => anyhow::bail!("Elevated command failed with exit code: {s}"),
                        Err(e2) => {
                            anyhow::bail!(
                                "Failed to run with sudo privileges.\n\
                                 \n\
                                 Error: {e2}\n\
                                 \n\
                                 For automation/CI, configure sudo with NOPASSWD:\n\
                                 sudo visudo\n\
                                 \n\
                                 And add line (replace username):\n\
                                 username ALL=(ALL) NOPASSWD: ALL\n\
                                 \n\
                                 Or specify this command specifically:\n\
                                 username ALL=(ALL) NOPASSWD: {}\n\
                                 \n\
                                 For interactive use, ensure you have sudo privileges.",
                                exe.display()
                            )
                        }
                    };
                }
                anyhow::bail!("Failed to elevate privileges: {e}")
            }
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

    #[test]
    fn test_mock_privilege_checker_not_root() {
        let checker = MockPrivilegeChecker::new();
        assert!(!checker.is_root());
    }

    #[test]
    fn test_mock_privilege_checker_set_root() {
        let mut checker = MockPrivilegeChecker::new();
        checker.set_root(true);
        assert!(checker.is_root());
    }

    #[test]
    fn test_mock_privilege_checker_elevation_allowed() {
        let mut checker = MockPrivilegeChecker::new();
        checker.set_elevation_allowed(true);
        let args = vec!["omg".to_string(), "install".to_string()];
        assert!(checker.elevate("install", &args).is_ok());
    }

    #[test]
    fn test_mock_privilege_checker_elevation_denied() {
        let mut checker = MockPrivilegeChecker::new();
        checker.set_elevation_allowed(false);
        let args = vec!["omg".to_string(), "install".to_string()];
        assert!(checker.elevate("install", &args).is_err());
    }

    #[test]
    fn test_mock_privilege_checker_logging() {
        let checker = MockPrivilegeChecker::new();
        let args = vec![
            "omg".to_string(),
            "install".to_string(),
            "firefox".to_string(),
        ];
        let _ = checker.elevate("install", &args);

        let log = checker.get_elevation_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].0, "install");
        assert_eq!(log[0].1, args);
    }

    #[test]
    fn test_system_privilege_checker() {
        let checker = SystemPrivilegeChecker;
        // Just ensure it doesn't panic
        let _ = checker.is_root();
    }

    #[test]
    fn test_global_privilege_checker() {
        let mock = std::sync::Arc::new(MockPrivilegeChecker::new());
        set_privilege_checker(mock.clone());

        let retrieved = get_privilege_checker();
        // The retrieved checker should work the same as the mock
        assert_eq!(retrieved.is_root(), mock.is_root());
    }

    #[test]
    fn test_all_allowed_operations_succeed() {
        let checker = MockPrivilegeChecker::new();
        let args = vec!["omg".to_string(), "install".to_string()];

        for op in ["install", "remove", "upgrade", "update", "sync", "clean"] {
            assert!(
                checker.elevate(op, &args).is_ok(),
                "Operation {} should succeed",
                op
            );
        }
    }

    #[test]
    fn test_security_rejection_for_dangerous_operations() {
        let args = vec!["omg".to_string()];
        // These should be rejected by the whitelist in elevate_for_operation
        for op in ["search", "info", "status", "evil_command", "rm -rf /"] {
            assert!(
                elevate_for_operation(op, &args).is_err(),
                "Operation {} should be rejected",
                op
            );
        }
    }
}
