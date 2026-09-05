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

    /// Create a card that keeps long lists readable while preserving the total.
    #[must_use]
    pub fn limited_card<M>(
        title: impl Into<String>,
        items: Vec<String>,
        visible_limit: usize,
    ) -> Cmd<M> {
        let omitted = items.len().saturating_sub(visible_limit);
        let mut visible: Vec<String> = items.into_iter().take(visible_limit).collect();
        if omitted > 0 {
            visible.push(format!("... and {omitted} more"));
        }
        Cmd::card(title, visible)
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

    /// Section header for grouping related output
    #[must_use]
    pub fn section<M>(title: impl Into<String>) -> Cmd<M> {
        Cmd::batch([Cmd::spacer(), Cmd::header(title.into(), ""), Cmd::spacer()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::tea::View;

    #[test]
    fn kv_list_with_title_is_a_card() {
        let cmd: Cmd<()> = Components::kv_list(Some("Info"), vec![("k", "v")]);
        match cmd {
            Cmd::View(View::Card(title, content)) => {
                assert_eq!(title, "Info");
                assert_eq!(content, vec!["k: v".to_string()]);
            }
            other => panic!("expected card, got {other:?}"),
        }
    }

    #[test]
    fn limited_card_reports_omitted_items() {
        let cmd: Cmd<()> = Components::limited_card(
            "Packages (4 total)",
            ["one", "two", "three", "four"].map(str::to_string).to_vec(),
            2,
        );
        match cmd {
            Cmd::View(View::Card(title, content)) => {
                assert_eq!(title, "Packages (4 total)");
                assert_eq!(content, ["one", "two", "... and 2 more"]);
            }
            other => panic!("expected card, got {other:?}"),
        }
    }
}
