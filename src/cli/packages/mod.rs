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
mod common;
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
pub use explicit::{explicit, explicit_sync};
pub use info::{info, info_aur, info_sync, info_sync_cli};
pub use install::install;
pub use remove::remove;
pub use search::{search, search_sync_cli};
pub use status::status;
pub use sync_db::sync_databases as sync;
pub use update::update;

/// Execute a `Cmd<()>` in fallback context (non-Elm mode)
///
/// This provides a simple println-based execution for reliability
/// in CI/non-TTY environments where the Elm UI might not be available.
pub fn execute_cmd(cmd: crate::cli::tea::Cmd<()>) {
    use crate::cli::tea::Cmd;
    use std::io::Write;

    fn execute_inner(cmd: Cmd<()>) {
        match cmd {
            Cmd::None => {}
            Cmd::Msg(_) => {
                // Messages don't make sense in fallback context
                // They're for Elm Architecture internal communication
            }
            Cmd::Batch(cmds) => {
                for c in cmds {
                    execute_inner(c);
                }
            }
            Cmd::Exec(_) => {
                // Exec closures would produce messages, not applicable here
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
                eprintln!("  ✗ {msg}");
            }
            Cmd::Header(title, body) => {
                println!("\n[{title}] {body}");
            }
            Cmd::Card(title, content) => {
                crate::cli::ui::print_card(&title, content);
            }
            Cmd::Progress(_) => {
                // Progress bars not supported in fallback mode
            }
            Cmd::Spinner(_) => {
                // Spinners not supported in fallback mode
            }
            Cmd::Table(_) => {
                // Tables not supported in fallback mode
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_cmd_basic() {
        // Test that execute_cmd handles basic commands without panicking
        use crate::cli::tea::Cmd;

        // Test simple print command
        execute_cmd(Cmd::print("test"));
        execute_cmd(Cmd::println("test line"));
        execute_cmd(Cmd::spacer());

        // Test styled commands
        execute_cmd(Cmd::info("info message"));
        execute_cmd(Cmd::success("success message"));
        execute_cmd(Cmd::warning("warning message"));
        execute_cmd(Cmd::error("error message"));

        // Test batch commands
        execute_cmd(Cmd::batch(vec![
            Cmd::println("line 1"),
            Cmd::println("line 2"),
            Cmd::spacer(),
        ]));

        // Test complex command with header and card
        execute_cmd(Cmd::header("Test", "Header body"));
        execute_cmd(Cmd::card(
            "Test Card",
            vec!["line 1".to_string(), "line 2".to_string()],
        ));

        // Test styled text
        execute_cmd(Cmd::StyledText(crate::cli::tea::StyledTextConfig {
            text: "bold text".to_string(),
            style: crate::cli::tea::TextStyle::Bold,
        }));

        // Test panel
        execute_cmd(Cmd::Panel(crate::cli::tea::PanelConfig {
            title: Some("Panel Title".to_string()),
            content: vec!["content line 1".to_string(), "content line 2".to_string()],
            border_style: crate::cli::tea::BorderStyle::Light,
            padding: 2,
        }));
    }

    #[test]
    fn test_execute_cmd_with_components() {
        // Test that execute_cmd works with Components helpers
        use crate::cli::components::Components;

        // Test error_with_suggestion
        execute_cmd(Components::error_with_suggestion(
            "Package not found",
            "Try searching with a different name",
        ));

        // Test update_summary
        execute_cmd(Components::update_summary(vec![
            ("pkg1", "1.0", "2.0"),
            ("pkg2", "1.5", "2.0"),
        ]));

        // Test no_results
        execute_cmd(Components::no_results("test query"));

        // Test up_to_date
        execute_cmd(Components::up_to_date());
    }
}
