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
pub use explicit::{explicit, explicit_sync, explicit_sync_with_json};
pub use info::{info, info_aur, info_sync, info_with_json};
pub use install::install;
pub use remove::remove;
pub use search::{search, search_sync_cli, search_sync_cli_with_limit, search_with_json};
pub use status::{status, status_with_json};
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

/// Execute a `Cmd<()>` in fallback context (non-Elm mode)
///
/// This provides a simple println-based execution for reliability
/// in CI/non-TTY environments where the Elm UI might not be available.
pub(crate) fn execute_cmd(cmd: crate::cli::tea::Cmd<()>) {
    use crate::cli::tea::Cmd;
    use std::io::Write;

    fn execute_inner(cmd: Cmd<()>) {
        match cmd {
            Cmd::None | Cmd::Msg(()) | Cmd::Exec(_) => {
                // Not supported or applicable in fallback mode
            }
            Cmd::Batch(cmds) => {
                for c in cmds {
                    execute_inner(c);
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
            Cmd::Error(msg) => {
                // User-facing failure: stderr so it stays visible even when
                // stdout is redirected or consumed by progress rendering.
                eprintln!("✗ {msg}");
            }
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
    }

    execute_inner(cmd);

    // Ensure output is flushed
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
}
