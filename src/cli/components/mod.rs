//! High-level UI components for the OMG CLI
//!
//! This module provides composite UI components that combine multiple
//! `Cmd` primitives for common patterns like loading states, error messages
//! with suggestions, and formatted lists.
//!
//! For basic output (success, error, info, etc.), use `Cmd` methods directly.
//! This module is for higher-level compositions that add semantic value.

use crate::cli::tea::{Cmd, StyledTextConfig, TextStyle};

/// High-level component builders for common UI patterns
///
/// Each method returns `Cmd<M>` where `M` is inferred from usage context.
/// Components work with any message type because they only produce output commands.
pub struct Components;

impl Components {
    /// Create a step indicator for multi-step processes
    ///
    /// Displays `[1/3] ⟳ Processing` for incomplete steps
    /// and `[3/3] ✓ Complete` for the final step.
    #[must_use]
    pub fn step<M>(step: usize, total: usize, message: impl Into<String>) -> Cmd<M> {
        let icon = if step == total { "✓" } else { "⟳" };
        let style = if step == total {
            TextStyle::Success
        } else {
            TextStyle::Info
        };

        Cmd::batch([
            Cmd::styled_text(StyledTextConfig {
                text: format!("[{step}/{total}] {icon}"),
                style,
            }),
            Cmd::println(format!(" {}", message.into())),
        ])
    }

    /// Create a formatted package list with numbering
    ///
    /// ```text
    /// ┌─ Available Packages ─┐
    /// │ 1. pkg-a - Description │
    /// │ 2. pkg-b - Description │
    /// └──────────────────────┘
    /// ```
    #[must_use]
    pub fn package_list<M>(
        title: impl Into<String>,
        packages: Vec<(impl Into<String>, Option<impl Into<String>>)>,
    ) -> Cmd<M> {
        let content: Vec<String> = packages
            .into_iter()
            .enumerate()
            .map(|(i, (name, desc))| {
                if let Some(d) = desc {
                    format!("{}. {} - {}", i + 1, name.into(), d.into())
                } else {
                    format!("{}. {}", i + 1, name.into())
                }
            })
            .collect();

        Cmd::card(title.into(), content)
    }

    /// Create an update summary showing version changes
    ///
    /// ```text
    /// ┌─ Updates Available ─┐
    /// │ pkg-a 1.0 → 2.0     │
    /// │ pkg-b 3.1 → 3.2     │
    /// └────────────────────┘
    /// ```
    #[must_use]
    pub fn update_summary<M>(
        packages: Vec<(impl Into<String>, impl Into<String>, impl Into<String>)>,
    ) -> Cmd<M> {
        let content: Vec<String> = packages
            .into_iter()
            .map(|(name, old_ver, new_ver)| {
                format!("{} {} → {}", name.into(), old_ver.into(), new_ver.into())
            })
            .collect();

        Cmd::card("Updates Available", content)
    }

    /// Create a key-value list, optionally in a card
    ///
    /// With title: renders as a card.
    /// Without title: renders as plain lines.
    #[must_use]
    pub fn kv_list<M>(
        title: Option<impl Into<String>>,
        items: Vec<(impl Into<String>, impl Into<String>)>,
    ) -> Cmd<M> {
        let content: Vec<String> = items
            .into_iter()
            .map(|(k, v)| format!("{}: {}", k.into(), v.into()))
            .collect();

        if let Some(t) = title {
            Cmd::card(t.into(), content)
        } else {
            // For untitled KV lists, just print each line
            content.into_iter().fold(Cmd::<M>::none(), |acc, c| {
                Cmd::batch(vec![acc, Cmd::println(c)])
            })
        }
    }

    /// Create a status summary (KV list with "Status" title)
    #[must_use]
    pub fn status_summary<M>(items: Vec<(impl Into<String>, impl Into<String>)>) -> Cmd<M> {
        Self::kv_list(Some("Status"), items)
    }
}


impl Components {
    /// Loading message with spinner icon
    ///
    /// ```text
    ///
    /// ℹ ⟳ Syncing repositories...
    ///
    /// ```
    #[must_use]
    pub fn loading<M>(message: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::info(format!("⟳ {}", message.into())),
            Cmd::spacer(),
        ])
    }

    /// "No results found" message with muted styling
    #[must_use]
    pub fn no_results<M>(query: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::styled_text(StyledTextConfig {
                text: format!("No results found for '{}'", query.into()),
                style: TextStyle::Muted,
            }),
            Cmd::spacer(),
        ])
    }

    /// "Already up to date" success message
    #[must_use]
    pub fn up_to_date<M>() -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::success("Everything is up to date!"),
            Cmd::spacer(),
        ])
    }

    /// Permission denied error with sudo suggestion
    #[must_use]
    pub fn permission_error<M>(command: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::error("Permission denied"),
            Cmd::styled_text(StyledTextConfig {
                text: format!("Try running: sudo {}", command.into()),
                style: TextStyle::Muted,
            }),
            Cmd::spacer(),
        ])
    }

    /// Confirmation prompt with action hint
    #[must_use]
    pub fn confirm<M>(message: impl Into<String>, action: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::bold(message.into()),
            Cmd::styled_text(StyledTextConfig {
                text: format!("Proceed? ({} or --yes to skip)", action.into()),
                style: TextStyle::Muted,
            }),
            Cmd::spacer(),
        ])
    }

    /// Command completed successfully with checkmark
    #[must_use]
    pub fn complete<M>(message: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::success(format!("✓ {}", message.into())),
            Cmd::spacer(),
        ])
    }

    /// Error message with actionable suggestion
    ///
    /// Displays error followed by a lightbulb icon and suggestion.
    #[must_use]
    pub fn error_with_suggestion<M>(
        error: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::error(error.into()),
            Cmd::info(format!("💡 {}", suggestion.into())),
            Cmd::spacer(),
        ])
    }

    /// Welcome banner for CLI commands
    #[must_use]
    pub fn welcome<M>(command: &str, description: &str) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::header(command, description),
            Cmd::spacer(),
        ])
    }

    /// Section header for grouping related output
    #[must_use]
    pub fn section<M>(title: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::header(title.into(), ""),
            Cmd::spacer(),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_indicator() {
        let cmd: Cmd<()> = Components::step(1, 3, "Processing");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_package_list() {
        let cmd: Cmd<()> =
            Components::package_list("Results", vec![("pkg1", Some("desc")), ("pkg2", None)]);
        assert!(matches!(cmd, Cmd::Card(_, _)));
    }

    #[test]
    fn test_update_summary() {
        let cmd: Cmd<()> = Components::update_summary(vec![("pkg", "1.0", "2.0")]);
        assert!(matches!(cmd, Cmd::Card(_, _)));
    }

    #[test]
    fn test_kv_list_with_title() {
        let cmd: Cmd<()> = Components::kv_list(Some("Info"), vec![("k", "v")]);
        assert!(matches!(cmd, Cmd::Card(_, _)));
    }

    #[test]
    fn test_kv_list_without_title() {
        let cmd: Cmd<()> = Components::kv_list::<()>(None::<&str>, vec![("k", "v")]);
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_status_summary() {
        let cmd: Cmd<()> = Components::status_summary(vec![("Status", "OK")]);
        assert!(matches!(cmd, Cmd::Card(_, _)));
    }

    #[test]
    fn test_loading() {
        let cmd: Cmd<()> = Components::loading("Processing");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_no_results() {
        let cmd: Cmd<()> = Components::no_results("test");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_up_to_date() {
        let cmd: Cmd<()> = Components::up_to_date();
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_permission_error() {
        let cmd: Cmd<()> = Components::permission_error("omg update");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_confirm() {
        let cmd: Cmd<()> = Components::confirm("Are you sure?", "Enter");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_complete() {
        let cmd: Cmd<()> = Components::complete("Done");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_error_with_suggestion() {
        let cmd: Cmd<()> = Components::error_with_suggestion("Error", "Fix it");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_welcome() {
        let cmd: Cmd<()> = Components::welcome("cmd", "desc");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_section() {
        let cmd: Cmd<()> = Components::section("Section");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }
}
