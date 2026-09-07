//! Privilege elevation utilities
//!
//! Package database writes run as root inside this binary (sudo re-exec), never
//! by spawning pacman or an AUR helper.

use std::sync::atomic::{AtomicBool, Ordering};

/// Privileged command lookup never consults the invoking user's PATH.
pub const SYSTEM_PATH: &str = "/usr/sbin:/usr/bin:/sbin:/bin";

/// Resolve an executable whose file and every ancestor are controlled by root.
/// Canonicalization permits the standard /bin -> /usr/bin merged layout.
pub fn trusted_program(program: &str) -> anyhow::Result<std::path::PathBuf> {
    let input = std::path::Path::new(program);
    let candidates = if input.is_absolute() {
        vec![input.to_path_buf()]
    } else {
        anyhow::ensure!(
            input.components().count() == 1,
            "Invalid system program: {program}"
        );
        SYSTEM_PATH
            .split(':')
            .map(|dir| std::path::Path::new(dir).join(input))
            .collect()
    };
    for candidate in candidates {
        if let Ok(path) = trusted_executable_path(&candidate) {
            return Ok(path);
        }
    }
    anyhow::bail!("No root-controlled system executable found for {program}")
}

fn trusted_executable_path(path: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let path = std::fs::canonicalize(path)?;
        let metadata = std::fs::metadata(&path)?;
        anyhow::ensure!(
            metadata.is_file() && metadata.mode() & 0o111 != 0,
            "Not executable"
        );
        for ancestor in path.ancestors() {
            let metadata = std::fs::metadata(ancestor)?;
            anyhow::ensure!(
                metadata.uid() == 0 && metadata.mode() & 0o022 == 0,
                "Executable path is writable by an unprivileged account: {}",
                ancestor.display()
            );
        }
        Ok(path)
    }
    #[cfg(not(unix))]
    anyhow::bail!(
        "Privilege elevation is unsupported on this platform: {}",
        path.display()
    )
}

pub fn system_command(program: &str) -> anyhow::Result<std::process::Command> {
    let mut command = std::process::Command::new(trusted_program(program)?);
    command.env("PATH", SYSTEM_PATH);
    for name in PRIVILEGED_ENV_SCRUB {
        command.env_remove(name);
    }
    Ok(command)
}

pub fn sudo_command() -> anyhow::Result<tokio::process::Command> {
    Ok(system_command("sudo")?.into())
}

/// Linux keeps the executing inode pinned, even if ~/.local/bin/omg is replaced.
/// Never canonicalize this proc path back to the mutable installation pathname.
fn elevation_executable() -> anyhow::Result<std::path::PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let path = std::path::PathBuf::from(format!("/proc/{}/exe", std::process::id()));
        std::fs::metadata(&path)?;
        Ok(path)
    }
    #[cfg(not(target_os = "linux"))]
    trusted_executable_path(&std::env::current_exe()?)
}

/// Environment variables stripped from every sudo child. One list so a new
/// scrub variable cannot be added in one elevation path and missed in another.
const PRIVILEGED_ENV_SCRUB: &[&str] = &[
    // Force terminal-based password prompt, never GUI askpass
    "SUDO_ASKPASS",
    "SSH_ASKPASS",
    "SSH_ASKPASS_REQUIRE",
    // Cargo build environment
    "CARGO_PRIMARY_PACKAGE",
    "CARGO_MANIFEST_DIR",
    "CARGO_TARGET_DIR",
    "CARGO_PKG_NAME",
    "CARGO_PKG_VERSION",
    "OUT_DIR",
    // Library injection vectors
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "LD_DEBUG",
    // Script execution vectors
    "PYTHONPATH",
    "RUBYLIB",
    "PERL5LIB",
    "NODE_PATH",
];

/// Strip [`PRIVILEGED_ENV_SCRUB`] from a sudo command builder.
fn scrub_privileged_env(command: &mut tokio::process::Command) {
    command.env("PATH", SYSTEM_PATH);
    for name in PRIVILEGED_ENV_SCRUB {
        command.env_remove(name);
    }
}
///
/// Elevation marker traveling through argv.
///
/// sudo's default `env_reset` strips `OMG_ELEVATED` from the child
/// environment. The child (see `src/bin/omg.rs` main) strips this marker
/// and sets `OMG_ELEVATED` itself before any dispatch. A non-root user
/// invoking the marker gains nothing: elevation checks still require
/// effective root.
pub const ELEVATED_MARKER: &str = "__omg_elevated";

/// Reserved argv token: mid-flow delegation whose PARENT owns the history record.
///
/// The parent appends this token because it has richer change metadata and AUR
/// handling; the elevated child strips it and skips its own
/// `record_fast_transaction` so each mutation is recorded exactly once.
/// Whole-command re-execs never carry it, so the child remains their sole
/// recorder.
pub const FLOW_PARENT_RECORDS: &str = "__omg_parent_records";

#[cfg(not(test))]
use std::sync::LazyLock;

#[cfg(not(test))]
use anyhow::Context;
#[cfg(not(test))]
use std::sync::Mutex;

/// Global mutex to serialize privilege elevation attempts
/// Prevents deadlocks when multiple threads try to elevate simultaneously
#[cfg(not(test))]
static ELEVATION_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Global flag to track if --yes was specified for non-interactive mode
static YES_FLAG: AtomicBool = AtomicBool::new(false);

// Set only by the validated root re-exec argv protocol. Full clap dispatches
// use it when flags prevent the minimal elevated path from owning history.
static PARENT_OWNS_HISTORY: AtomicBool = AtomicBool::new(false);

#[doc(hidden)]
pub fn set_parent_owns_history(value: bool) {
    PARENT_OWNS_HISTORY.store(value, Ordering::SeqCst);
}

#[doc(hidden)]
#[must_use]
pub fn parent_owns_history() -> bool {
    PARENT_OWNS_HISTORY.load(Ordering::SeqCst)
}

/// Set the yes flag globally (call this at the start of main if --yes is present)
pub fn set_yes_flag(value: bool) {
    YES_FLAG.store(value, Ordering::SeqCst);
}

/// Run an EXTERNAL program under sudo as one step inside a larger flow.
///
/// Unlike [`run_privileged_child`] this never re-executes omg: the native
/// package manager (apt-get, dnf) runs directly with explicit arguments, so
/// there is exactly one prompt and no re-listing/re-confirming of work the
/// caller already resolved. Credentials are validated with a pre-flight
/// `sudo -n -v`; when that fails and stdin is interactive, one authentication
/// prompt is offered before giving up.
///
/// # Errors
/// Dev/test mode bails without touching sudo. A password requirement in a
/// non-interactive session, or a nonzero child status, is returned as an
/// error.
fn reject_privileged_program_in_dev_mode(
    dev_mode: bool,
    program: &str,
    args: &[&str],
) -> anyhow::Result<()> {
    anyhow::ensure!(
        !dev_mode,
        "Privilege elevation not supported in development mode.\n\
         \n\
         Options:\n\
         • Prime sudo credentials: omg doctor --turbo\n\
         • Run directly with sudo: sudo {program} {args:?}"
    );
    Ok(())
}

pub async fn run_privileged_program(program: &str, args: &[&str]) -> anyhow::Result<()> {
    // Detect dev/test mode — identical contract to run_self_sudo.
    reject_privileged_program_in_dev_mode(
        crate::core::paths::test_mode() || std::env::var("CARGO_PRIMARY_PACKAGE").is_ok(),
        program,
        args,
    )?;

    if matches!(
        std::path::Path::new(program)
            .file_name()
            .and_then(|name| name.to_str()),
        Some("apt-get" | "dnf")
    ) && args.first().is_some_and(|arg| {
        matches!(
            *arg,
            "install" | "upgrade" | "dist-upgrade" | "full-upgrade"
        )
    }) {
        crate::core::security::policy::require_native_plan_support(program)?;
    }
    let program_path = trusted_program(program)?;

    // Pre-flight: validate/refresh credentials WITHOUT running the payload,
    // so a password requirement is detected before any partial work.
    let authenticated = sudo_command()?
        .arg("-n")
        .arg("-v")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok_and(|s| s.success());

    if !authenticated {
        if get_yes_flag() || !console::user_attended() {
            anyhow::bail!(
                "Privilege elevation requires a password but no interactive terminal is available.\n\
                 \n\
                 Run 'omg doctor --turbo' interactively to prime sudo credentials,\n\
                 and use your administrator-approved sudo policy for automation."
            );
        }
        // One interactive authentication prompt with inherited stdio.
        let mut auth_cmd = sudo_command()?;
        scrub_privileged_env(&mut auth_cmd);
        let status = auth_cmd
            .arg("-v")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to run sudo for credential validation: {e}"))?;
        if !status.success() {
            anyhow::bail!("sudo authentication failed");
        }
    }

    let audit_targets = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    crate::core::security::audit::record_operation(program, &audit_targets, "attempt")?;
    let mut elevated = sudo_command()?;
    scrub_privileged_env(&mut elevated);
    let status = elevated
        .arg("--")
        .arg(program_path)
        .args(args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to run {program} under sudo: {e}"))?;

    crate::core::security::audit::record_operation(
        program,
        &audit_targets,
        if status.success() {
            "succeeded"
        } else {
            "failed"
        },
    )?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!(
            "{program} failed with exit code {}",
            status.code().unwrap_or(1)
        )
    }
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
        #[cfg(unix)]
        {
            rustix::process::geteuid().is_root()
        }
        #[cfg(not(unix))]
        {
            false
        }
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
        self.elevation_log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
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
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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

/// Check if we're running as root
#[must_use]
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        rustix::process::geteuid().is_root()
    }

    #[cfg(not(unix))]
    {
        false
    }
}

/// Re-execute the current command with sudo if not root
/// This replaces the current process - it doesn't return on success
pub fn elevate_if_needed(args: &[String]) -> anyhow::Result<()> {
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
        // Acquire lock before elevation to prevent concurrent sudo attempts
        let _guard = ELEVATION_MUTEX
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let yes_mode = YES_FLAG.load(Ordering::Relaxed);
        tracing::debug!(
            "Not running as root, attempting elevation. yes_mode={}",
            yes_mode
        );

        // Skip argv[0] as run_self_sudo adds the executable path itself
        let args_refs: Vec<&str> = args
            .iter()
            .skip(1)
            .map(std::string::String::as_str)
            .collect();

        let run_elevation = |args_refs: &[&str]| -> anyhow::Result<()> {
            tokio::runtime::Runtime::new()
                .context("Failed to create runtime")?
                .block_on(run_self_sudo(args_refs))
        };

        // Creating a runtime and calling block_on panics when invoked from
        // within an existing async runtime. When we are inside one, isolate
        // the elevation on a dedicated thread with its own runtime instead.
        if tokio::runtime::Handle::try_current().is_ok() {
            std::thread::scope(|scope| {
                scope
                    .spawn(|| run_elevation(&args_refs))
                    .join()
                    .map_err(|_| anyhow::anyhow!("elevation thread panicked"))?
            })?;
        } else {
            run_elevation(&args_refs)?;
        }

        // If run_self_sudo returns, it means the command succeeded.
        // We exit here to mimic exec() behavior (process replacement)
        std::process::exit(0);
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

    elevate_if_needed(args).map_err(std::io::Error::other)
}

/// Run the current executable with sudo and specific arguments asynchronously
/// Build a scrubbed sudo command that re-executes the current binary.
///
/// Note: We set `OMG_ELEVATED=1` as an environment variable for the child
/// process. This prevents infinite recursion when the elevated process checks
/// this flag.
///
/// CRITICAL: Remove CARGO_* environment variables to prevent the elevated
/// process from writing to the user's target directory as root (causing
/// permission errors).
///
/// SECURITY: Also remove dangerous environment variables that could be used
/// to hijack library loading or script execution in an elevated context.
/// Askpass variables are removed to force the terminal-based prompt.
fn payload_command(
    sudo_program: &std::path::Path,
    exe: &std::path::Path,
    args: &[&str],
    non_interactive: bool,
) -> tokio::process::Command {
    let parent_records = args.last().copied() == Some(FLOW_PARENT_RECORDS);
    let payload_args = if parent_records {
        &args[..args.len() - 1]
    } else {
        args
    };
    let mut command = tokio::process::Command::new(sudo_program);
    if non_interactive {
        // -n fails immediately if a password would be required
        command.arg("-n");
    }
    command
        // Elevation is transmitted via an ARGV MARKER, not the environment:
        // sudo's default env_reset strips OMG_ELEVATED from the child, which
        // made every fast-elevated path dead code. The child strips this
        // marker and sets OMG_ELEVATED itself before any dispatch.
        .env("OMG_ELEVATED", "1");
    scrub_privileged_env(&mut command);
    command
        // Inherit terminal so output and any password prompt stay attached
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .arg("--")
        .arg(exe)
        .arg(crate::core::privilege::ELEVATED_MARKER);
    if parent_records {
        // Internal flow ownership is positional protocol metadata, never a
        // package-list token. The root child accepts it only immediately
        // after the authenticated elevation marker.
        command.arg(FLOW_PARENT_RECORDS);
    }
    command.args(payload_args);
    command
}

/// Run the omg payload under sudo exactly once and return its exit status
/// without terminating this process.
///
/// In-flow callers (composite operations such as "sync then list then
/// upgrade") must use [`run_privileged_child`] so work after the elevated
/// step still runs. Only whole-process re-exec points should use
/// [`run_self_sudo`], which exits to mimic exec() semantics.
async fn sudo_payload_status(args: &[&str]) -> anyhow::Result<std::process::ExitStatus> {
    let exe = elevation_executable()?;

    // Detect if we're running in development/test mode.
    let is_test_mode =
        crate::core::paths::test_mode() || std::env::var("CARGO_PRIMARY_PACKAGE").is_ok();
    let owned_args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    let pinned = if args.first() == Some(&"install") {
        Some(crate::core::security::artifact::SnapshotInputs::capture(
            &owned_args,
        )?)
    } else {
        None
    };
    let policy = crate::core::security::policy::policy_handoff()?;
    let mut args: Vec<_> = pinned
        .as_ref()
        .map_or(owned_args.as_slice(), |inputs| inputs.targets.as_slice())
        .iter()
        .map(String::as_str)
        .collect();
    if let Some(policy) = &policy {
        args.insert(0, policy);
    }
    sudo_payload_status_in(&trusted_program("sudo")?, exe, is_test_mode, &args).await
}

/// Dev-mode-injectable core of [`sudo_payload_status`]: `dev_mode` short-
/// circuits before any sudo invocation so tests never touch real sudo.
async fn sudo_payload_status_in(
    sudo_program: &std::path::Path,
    exe: std::path::PathBuf,
    dev_mode: bool,
    args: &[&str],
) -> anyhow::Result<std::process::ExitStatus> {
    if dev_mode {
        anyhow::bail!(
            "Privilege elevation not supported in development mode.\n\
             \n\
             Options:\n\
             • Prime sudo credentials: omg doctor --turbo\n\
             • Run directly with sudo: sudo {} {:?}",
            exe.display(),
            args
        );
    }

    // Check if --yes flag is set for non-interactive mode
    let yes_flag = get_yes_flag();

    // Correctness: validate sudo authentication BEFORE running the payload.
    // Without this pre-flight, a payload command failing under cached
    // credentials (sudo -n executes it directly) was misattributed to "password
    // required" and the entire privileged operation was silently re-executed,
    // repeating its side effects. The validation only refreshes the sudo
    // timestamp; it never runs the payload.
    let mut preflight = tokio::process::Command::new(sudo_program);
    scrub_privileged_env(&mut preflight);
    let validated = preflight
        .arg("-n")
        .arg("-v")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;

    let authenticated = match validated {
        Ok(status) if status.success() => true,
        Ok(_) => false,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Failed to run sudo for privilege elevation: {e}\n\
                 \n\
                 Run 'omg doctor --turbo' interactively to prime sudo credentials,\n\
                 and use your administrator-approved sudo policy for automation."
            ));
        }
    };

    if !authenticated {
        // sudo cannot authenticate non-interactively: a password is needed.
        if yes_flag {
            return Err(anyhow::anyhow!(
                "Privilege elevation failed (--yes flag prevents password prompt).\n\
                 \n\
                 Run 'omg doctor --turbo' interactively to prime sudo credentials.\n\
                 For automation, use an administrator-approved authenticated session.\n\
                 \n\
                 Alternative: remove --yes to allow a password prompt.\n\
                 Current user: {user}; omg executable: {exe}",
                user = whoami::username().unwrap_or_else(|_| "username".to_string()),
                exe = exe.display()
            ));
        }

        // Interactive sudo WITHOUT timeout. IMPORTANT: stdin/stdout/stderr are
        // inherited by `payload_command` so the password prompt stays in the
        // terminal instead of spawning a GUI askpass dialog. The user can
        // Ctrl+C if needed. Runs exactly once.
        tracing::debug!("Password required, running interactive sudo");
        return payload_command(sudo_program, &exe, args, false)
            .status()
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to run with sudo privileges: {e}\n\
                 \n\
                 Run 'omg doctor --turbo' interactively to prime sudo credentials\n\
                 before retrying."
                )
            });
    }

    // Authenticated non-interactively: run the payload exactly once and
    // propagate its result. Never retried — a failing elevated command must
    // surface its own error, not trigger a second execution.
    payload_command(sudo_program, &exe, args, true)
        .status()
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to elevate privileges: {e}\n\
             \n\
             Run 'omg doctor --turbo' interactively to prime sudo credentials\n\
             before retrying."
            )
        })
}

/// Re-execute the current command under sudo, replacing this process.
///
/// Does not return on success: the elevated child handles the entire
/// command. Use only where the privileged operation IS the whole command
/// (top-of-main elevation, `elevate_if_needed`). For an elevated step inside
/// a larger flow, use [`run_privileged_child`].
pub async fn run_self_sudo(args: &[&str]) -> anyhow::Result<()> {
    let status = sudo_payload_status(args).await?;
    std::process::exit(status.code().unwrap_or(1));
}

/// Run the current command under sudo as one step inside a larger operation,
/// returning once the elevated child finishes.
///
/// Unlike [`run_self_sudo`] this never terminates the calling process:
/// success returns `Ok(())`, a nonzero child status is returned as an error,
/// and the caller continues with the rest of the flow (listing updates,
/// building AUR packages, recording history, printing summaries).
pub async fn run_privileged_child(args: &[&str]) -> anyhow::Result<()> {
    let status = sudo_payload_status(args).await?;
    if status.success() {
        return Ok(());
    }
    anyhow::bail!("Elevated command failed with exit code: {status}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fake_sudo(
        validation_exit: i32,
        payload_exit: i32,
    ) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("fake sudo tempdir");
        let sudo = directory.path().join("sudo");
        let log = directory.path().join("sudo.log");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\n\
             if [ \"$1\" = '-n' ] && [ \"$2\" = '-v' ]; then exit {validation_exit}; fi\n\
             exit {payload_exit}\n",
            log.display()
        );
        std::fs::write(&sudo, script).expect("write fake sudo");
        let mut permissions = std::fs::metadata(&sudo)
            .expect("fake sudo metadata")
            .permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&sudo, permissions).expect("chmod fake sudo");
        (directory, sudo, log)
    }

    struct YesFlagReset;

    impl Drop for YesFlagReset {
        fn drop(&mut self) {
            set_yes_flag(false);
        }
    }

    #[test]
    fn payload_command_transmits_elevation_via_argv_marker() {
        // Regression: OMG_ELEVATED=1 in the child environment is stripped by
        // sudo's env_reset, which killed every fast-elevated path. The marker
        // must be part of argv instead.
        let exe = std::path::PathBuf::from("/usr/bin/omg");
        let command = payload_command(
            std::path::Path::new("sudo"),
            &exe,
            &["fullupdate", "--"],
            true,
        );
        let argv = command.as_std().get_args().collect::<Vec<_>>();
        let marker_pos = argv
            .iter()
            .position(|a| *a == ELEVATED_MARKER)
            .expect("elevation marker missing from sudo payload argv");
        // Layout: sudo … --  <exe>  <MARKER>  <payload args…>
        assert_eq!(argv[marker_pos - 1], "/usr/bin/omg");
        assert_eq!(argv[marker_pos + 1], "fullupdate");
        assert_eq!(argv[marker_pos + 2], "--");
    }

    #[test]
    fn payload_command_moves_history_ownership_out_of_package_arguments() {
        let exe = std::path::PathBuf::from("/usr/bin/omg");
        let command = payload_command(
            std::path::Path::new("sudo"),
            &exe,
            &["install", "--", "ripgrep", FLOW_PARENT_RECORDS],
            true,
        );
        let argv = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let marker = argv
            .iter()
            .position(|argument| argument == ELEVATED_MARKER)
            .expect("elevation marker");

        assert_eq!(
            argv.get(marker + 1).map(String::as_str),
            Some(FLOW_PARENT_RECORDS)
        );
        assert_eq!(argv.get(marker + 2).map(String::as_str), Some("install"));
        assert_eq!(argv.last().map(String::as_str), Some("ripgrep"));
    }

    #[tokio::test]
    async fn elevation_bails_in_dev_mode_instead_of_exiting() {
        // Contract: dev-mode elevation surfaces an error and RETURNS rather
        // than terminating the process, so composite flows keep executing.
        // The injectable core guarantees no real sudo invocation occurs.
        let exe = std::env::current_exe().expect("current exe");
        let result =
            sudo_payload_status_in(std::path::Path::new("sudo"), exe, true, &["sync"]).await;
        let err = result.expect_err("dev-mode elevation must fail closed, not exit");
        assert!(err.to_string().contains("development mode"));
    }

    #[tokio::test]
    async fn cached_credentials_run_one_noninteractive_payload() {
        let (_directory, sudo, log) = fake_sudo(0, 7);
        let status = sudo_payload_status_in(
            &sudo,
            std::path::PathBuf::from("/usr/bin/omg"),
            false,
            &["sync"],
        )
        .await
        .expect("fake sudo should execute");

        assert_eq!(status.code(), Some(7), "payload status must propagate");
        let invocations = std::fs::read_to_string(log).expect("read fake sudo log");
        let lines = invocations.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2, "preflight plus exactly one payload");
        assert_eq!(lines[0], "-n -v");
        assert_eq!(
            lines[1],
            format!("-n -- /usr/bin/omg {ELEVATED_MARKER} sync"),
            "cached credentials must keep the payload noninteractive"
        );
    }

    #[test]
    fn privileged_program_dev_mode_rejection_is_actionable() {
        let error = reject_privileged_program_in_dev_mode(true, "apt-get", &["update"])
            .expect_err("development mode must reject external elevation");
        let message = error.to_string();
        assert!(message.contains("development mode"));
        assert!(message.contains("apt-get"));
        assert!(message.contains("sudo"));
    }

    #[tokio::test]
    async fn failed_preflight_with_yes_never_runs_payload() {
        set_yes_flag(true);
        let _reset = YesFlagReset;
        let (_directory, sudo, log) = fake_sudo(1, 0);
        let error = sudo_payload_status_in(
            &sudo,
            std::path::PathBuf::from("/usr/bin/omg"),
            false,
            &["sync"],
        )
        .await
        .expect_err("--yes must reject a password-requiring preflight");

        assert!(
            error
                .to_string()
                .contains("--yes flag prevents password prompt")
        );
        assert_eq!(
            std::fs::read_to_string(log).expect("read fake sudo log"),
            "-n -v\n",
            "failed preflight must not execute the payload"
        );
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
    fn test_all_allowed_operations_succeed() {
        let checker = MockPrivilegeChecker::new();
        let args = vec!["omg".to_string(), "install".to_string()];

        for op in ["install", "remove", "upgrade", "update", "sync", "clean"] {
            assert!(
                checker.elevate(op, &args).is_ok(),
                "Operation {op} should succeed"
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
                "Operation {op} should be rejected"
            );
        }
    }
}

#[cfg(all(test, unix))]
mod trusted_program_tests {
    use super::*;
    #[test]
    fn lookup_ignores_hostile_path_and_rejects_writable_program() -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir()?;
        let fake = directory.path().join("sudo");
        std::fs::write(&fake, "#!/bin/sh\nexit 77\n")?;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755))?;
        assert!(trusted_executable_path(&fake).is_err());
        let command = system_command("sudo")?;
        assert!(std::path::Path::new(command.get_program()).is_absolute());
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == "PATH")
                .unwrap()
                .1
                .unwrap(),
            SYSTEM_PATH
        );
        Ok(())
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn elevation_uses_the_running_inode() -> anyhow::Result<()> {
        use std::os::unix::fs::MetadataExt;
        let pinned = elevation_executable()?;
        assert_eq!(
            pinned,
            std::path::PathBuf::from(format!("/proc/{}/exe", std::process::id()))
        );
        assert_eq!(
            std::fs::metadata(pinned)?.ino(),
            std::fs::metadata(std::env::current_exe()?)?.ino()
        );
        Ok(())
    }
}
