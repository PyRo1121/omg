//! Package management CLI operations
//!
//! This module provides all package-related CLI functionality:
//! - Search: Find packages in repositories and AUR
//! - Install: Install packages with security grading
//! - Remove: Uninstall packages
//! - Update: System-wide package updates
//! - Info: Display package information
//! - Clean: Remove orphans and clear caches
//! - Explicit: List explicitly installed packages
//! - Sync: Synchronize package databases

mod clean;
pub(crate) mod common;
mod explicit;
mod info;
mod install;
mod remove;
mod search;
mod status;
mod sync_db;
mod update;

// Re-export all public functions
pub use clean::clean;
pub use explicit::{explicit_sync, explicit_sync_with_json};
pub use info::{info_sync, info_with_json};
pub use install::install;
pub use remove::remove;
pub use search::{search_sync_cli_with_limit, search_with_json};
pub use status::status_with_json;
pub use sync_db::sync_databases as sync;
pub use update::{update, update_fast, update_turbo};

/// Dispatch to the compiled package-manager backend.
///
/// Shared backend-selection policy for the install/remove/update commands:
///
/// 1. Debian-like distros use the Debian backend when Debian support is
///    compiled in.
/// 2. Otherwise the Arch backend is preferred when compiled in.
/// 3. With Debian support but no Arch support, Debian is the fallback.
/// 4. Otherwise the generic backend is used.
///
/// Each body is a block expression. Only the arms enabled by the active
/// feature flags are type-checked, so call sites may reference the backend
/// modules unconditionally.
macro_rules! dispatch_backend {
    (
        debian: $debian_body:block,
        arch: $arch_body:block,
        generic: $generic_body:block $(,)?
    ) => {
        #[cfg(any(feature = "debian", feature = "debian-pure"))]
        if crate::core::env::distro::is_debian_like() {
            return $debian_body;
        }

        #[cfg(feature = "arch")]
        $arch_body

        #[cfg(all(
            not(feature = "arch"),
            any(feature = "debian", feature = "debian-pure")
        ))]
        $debian_body

        #[cfg(all(
            not(feature = "arch"),
            not(any(feature = "debian", feature = "debian-pure"))
        ))]
        $generic_body
    };
}
pub(crate) use dispatch_backend;

/// Execute a `Cmd<()>` in fallback context (non-Elm mode).
///
/// This provides a simple println-based execution for reliability
/// in CI/non-TTY environments where the Elm UI might not be available.
/// [`Cmd::Error`] is returned without printing so the process-level reporter
/// remains the single owner of user-facing failures.
pub(crate) fn execute_cmd(cmd: crate::cli::tea::Cmd<()>) -> anyhow::Result<()> {
    use crate::cli::tea::Cmd;
    use std::io::Write;

    fn execute_inner(cmd: Cmd<()>) -> anyhow::Result<()> {
        match cmd {
            Cmd::None | Cmd::Msg(()) | Cmd::Exec(_) => {
                // Not supported or applicable in fallback mode
            }
            Cmd::Batch(cmds) => {
                let mut first_error = None;
                for command in cmds {
                    if let Err(error) = execute_inner(command)
                        && first_error.is_none()
                    {
                        first_error = Some(error);
                    }
                }
                if let Some(error) = first_error {
                    return Err(error);
                }
            }
            Cmd::PrintLn(output) => {
                println!("{output}");
            }
            Cmd::Info(msg) => {
                println!("  ℹ {msg}");
            }
            Cmd::Success(msg) => {
                println!("  ✓ {msg}");
            }
            Cmd::Warning(msg) => {
                println!("  ⚠ {msg}");
            }
            Cmd::Error(msg) => anyhow::bail!("{msg}"),
            Cmd::Header(title, body) => {
                println!("\n[{title}] {body}");
            }
            Cmd::Card(title, content) => {
                crate::cli::ui::print_card(&title, content);
            }
            Cmd::StyledText(config) => {
                // In fallback mode, just print the text without styling
                println!("{}", config.text);
            }
            Cmd::Spacer => {
                println!();
            }
        }
        Ok(())
    }

    execute_inner(cmd)?;

    // Ensure output is flushed.
    std::io::stdout().flush()?;
    std::io::stderr().flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::execute_cmd;
    use crate::cli::tea::Cmd;

    #[test]
    fn fallback_executor_propagates_cmd_errors() {
        let error = execute_cmd(Cmd::error("package operation failed"))
            .expect_err("fallback Cmd::Error must fail the command");
        assert!(error.to_string().contains("package operation failed"));
    }
}
