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
pub mod common;
mod explicit;
mod info;
mod install;
pub mod local;
mod remove;
mod search;
mod status;
mod sync_db;
mod update;

// Re-export all public functions
pub use clean::clean;
pub use explicit::{explicit, explicit_sync, explicit_sync_with_json};
pub use info::{info, info_aur, info_sync, info_sync_cli, info_with_json};
pub use install::{install, install_dry_run_cli};
pub use remove::remove;
pub use search::{search, search_sync_cli, search_sync_cli_with_limit, search_with_json};
pub use status::{status, status_with_json};
pub use sync_db::sync_databases as sync;
pub use update::{update, update_fast, update_turbo};

/// Execute a `Cmd<()>` in fallback context (non-Elm mode)
///
/// This provides a simple println-based execution for reliability
/// in CI/non-TTY environments where the Elm UI might not be available.
pub fn execute_cmd(cmd: crate::cli::tea::Cmd<()>) {
    use crate::cli::tea::Cmd;
    use std::io::Write;

    fn execute_inner(cmd: Cmd<()>) {
        match cmd {
            Cmd::None
            | Cmd::Msg(())
            | Cmd::Exec(_)
            | Cmd::Progress(_)
            | Cmd::Spinner(_)
            | Cmd::Table(_) => {
                // Not supported or applicable in fallback mode
            }
            Cmd::Batch(cmds) => {
                for c in cmds {
                    execute_inner(c);
                }
            }
            Cmd::Print(output) => {
                print!("{output}");
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
                tracing::error!("{}", msg);
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
            Cmd::Panel(config) => {
                if let Some(title) = &config.title {
                    println!("\n[{title}]");
                }
                for line in &config.content {
                    println!("{}{}", " ".repeat(config.padding), line);
                }
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
