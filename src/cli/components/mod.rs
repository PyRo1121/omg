//! Reusable UI components for the OMG CLI
//!
//! This module provides pre-built, styled UI components that can be used
//! across all CLI commands for consistent visual design.

use crate::cli::tea::Cmd;

/// Component library for reusable UI elements
pub struct Components;

impl Components {
    /// Create a styled header for command output
    #[must_use]
    pub fn header<M>(title: impl Into<String>, body: impl Into<String>) -> Cmd<M> {
        Cmd::header(title.into(), body.into())
    }

    /// Create a success message with icon
    #[must_use]
    pub fn success<M>(message: impl Into<String>) -> Cmd<M> {
        Cmd::success(message.into())
    }

    /// Create an error message with icon
    #[must_use]
    pub fn error<M>(message: impl Into<String>) -> Cmd<M> {
        Cmd::error(message.into())
    }

    /// Create a warning message with icon
    #[must_use]
    pub fn warning<M>(message: impl Into<String>) -> Cmd<M> {
        Cmd::warning(message.into())
    }

    /// Create an info message with icon
    #[must_use]
    pub fn info<M>(message: impl Into<String>) -> Cmd<M> {
        Cmd::info(message.into())
    }

    /// Create a bordered card with title and content
    #[must_use]
    pub fn card<M>(title: impl Into<String>, content: Vec<String>) -> Cmd<M> {
        Cmd::card(title.into(), content)
    }

    /// Print a blank line (spacer)
    #[must_use]
    pub fn spacer<M>() -> Cmd<M> {
        Cmd::spacer()
    }

    /// Create bold text
    #[must_use]
    pub fn bold<M>(text: impl Into<String>) -> Cmd<M> {
        Cmd::styled_text(crate::cli::tea::StyledTextConfig {
            text: text.into(),
            style: crate::cli::tea::TextStyle::Bold,
        })
    }

    /// Create muted/gray text
    #[must_use]
    pub fn muted<M>(text: impl Into<String>) -> Cmd<M> {
        Cmd::styled_text(crate::cli::tea::StyledTextConfig {
            text: text.into(),
            style: crate::cli::tea::TextStyle::Muted,
        })
    }

    /// Create a step indicator for multi-step processes
    #[must_use]
    pub fn step<M>(step: usize, total: usize, message: impl Into<String>) -> Cmd<M> {
        let icon = if step == total { "✓" } else { "⟳" };
        let style = if step == total {
            crate::cli::tea::TextStyle::Success
        } else {
            crate::cli::tea::TextStyle::Info
        };

        Cmd::batch([
            Cmd::styled_text(crate::cli::tea::StyledTextConfig {
                text: format!("[{}/{}] {}", step, total, icon),
                style,
            }),
            Cmd::println(format!(" {}", message.into())),
        ])
    }

    /// Create a package list for search/install results
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

    /// Create an update summary showing package updates
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

    /// Create a key-value list panel
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

    /// Create a status summary
    #[must_use]
    pub fn status_summary<M>(items: Vec<(impl Into<String>, impl Into<String>)>) -> Cmd<M> {
        Self::kv_list(Some("Status"), items)
    }
}

/// Pre-built components for common use cases
impl Components {
    /// Loading message for async operations
    #[must_use]
    pub fn loading<M>(message: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::info(format!("⟳ {}", message.into())),
            Cmd::spacer(),
        ])
    }

    /// "No results found" message
    #[must_use]
    pub fn no_results<M>(query: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Self::muted(format!("No results found for '{}'", query.into())),
            Cmd::spacer(),
        ])
    }

    /// "Already up to date" message
    #[must_use]
    pub fn up_to_date<M>() -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::success("Everything is up to date!"),
            Cmd::spacer(),
        ])
    }

    /// Permission error message with suggestion
    #[must_use]
    pub fn permission_error<M>(command: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::error("Permission denied"),
            Self::muted(format!("Try running: sudo {}", command.into())),
            Cmd::spacer(),
        ])
    }

    /// Confirmation prompt for operations
    #[must_use]
    pub fn confirm<M>(message: impl Into<String>, action: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Self::bold(message.into()),
            Self::muted(format!("Proceed? ({} or --yes to skip)", action.into())),
            Cmd::spacer(),
        ])
    }

    /// Command completed successfully message
    #[must_use]
    pub fn complete<M>(message: impl Into<String>) -> Cmd<M> {
        Cmd::batch([
            Cmd::spacer(),
            Cmd::success(format!("✓ {}", message.into())),
            Cmd::spacer(),
        ])
    }

    /// Error with suggestion for remediation
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

    /// Welcome/header banner for the CLI
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
    fn test_components_header() {
        let cmd: Cmd<()> = Components::header("Test", "Body");
        assert!(matches!(cmd, Cmd::Header(_, _)));
    }

    #[test]
    fn test_components_success() {
        let cmd: Cmd<()> = Components::success("Done");
        assert!(matches!(cmd, Cmd::Success(_)));
    }

    #[test]
    fn test_components_spacer() {
        let cmd: Cmd<()> = Components::spacer();
        assert!(matches!(cmd, Cmd::Spacer));
    }

    #[test]
    fn test_components_bold() {
        let cmd: Cmd<()> = Components::bold("Important");
        assert!(matches!(cmd, Cmd::StyledText(_)));
    }

    #[test]
    fn test_components_step() {
        let cmd: Cmd<()> = Components::step(1, 3, "Processing");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_components_package_list() {
        let cmd: Cmd<()> = Components::package_list(
            "Results",
            vec![("pkg1", Some("desc")), ("pkg2", None)],
        );
        assert!(matches!(cmd, Cmd::Card(_, _)));
    }

    #[test]
    fn test_components_update_summary() {
        let cmd: Cmd<()> = Components::update_summary(vec![("pkg", "1.0", "2.0")]);
        assert!(matches!(cmd, Cmd::Card(_, _)));
    }

    #[test]
    fn test_components_kv_list() {
        let cmd: Cmd<()> = Components::kv_list(Some("Info"), vec![("k", "v")]);
        assert!(matches!(cmd, Cmd::Card(_, _)));
    }

    #[test]
    fn test_components_no_results() {
        let cmd: Cmd<()> = Components::no_results("test");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_components_up_to_date() {
        let cmd: Cmd<()> = Components::up_to_date();
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_components_permission_error() {
        let cmd: Cmd<()> = Components::permission_error("omg update");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_components_complete() {
        let cmd: Cmd<()> = Components::complete("Done");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_components_error_with_suggestion() {
        let cmd: Cmd<()> = Components::error_with_suggestion("Error", "Fix it");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_components_welcome() {
        let cmd: Cmd<()> = Components::welcome("cmd", "desc");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }

    #[test]
    fn test_components_section() {
        let cmd: Cmd<()> = Components::section("Section");
        assert!(matches!(cmd, Cmd::Batch(_)));
    }
}
