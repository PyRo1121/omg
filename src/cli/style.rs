//! Consistent styling utilities for OMG CLI output
//!
//! All output should use these helpers for consistent UX.
//!
//! ## Features
//! - **`NO_COLOR` support**: Respects the [NO_COLOR standard](https://no-color.org/)
//! - **TTY detection**: Auto-detects terminal capabilities
//! - **Accessibility**: WCAG AA compliant contrast ratios

use std::env;
#[cfg(not(test))]
use std::sync::OnceLock;

use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use supports_color::Stream;

// ═══════════════════════════════════════════════════════════════════════════
// COLOR DETECTION & CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(not(test))]
static COLORS_ENABLED_CACHE: OnceLock<bool> = OnceLock::new();
#[cfg(not(test))]
static USE_UNICODE_CACHE: OnceLock<bool> = OnceLock::new();

/// Detect if colors should be enabled
///
/// Follows the [NO_COLOR standard](https://no-color.org/) and detects TTY support.
#[must_use]
pub fn colors_enabled() -> bool {
    #[cfg(test)]
    {
        detect_colors_enabled()
    }

    #[cfg(not(test))]
    {
        *COLORS_ENABLED_CACHE.get_or_init(detect_colors_enabled)
    }
}

fn detect_colors_enabled() -> bool {
    if env::var("NO_COLOR").is_ok() {
        return false;
    }

    // 2. Check if OMG_COLORS is explicitly disabled
    if let Ok(val) = env::var("OMG_COLORS") {
        if matches!(val.as_str(), "never" | "0" | "false") {
            return false;
        }
        if matches!(val.as_str(), "always" | "1" | "true") {
            return true;
        }
    }

    // 3. Check terminal capabilities via supports-color crate
    supports_color::on(Stream::Stdout).is_some_and(|level| level.has_basic)
}

/// Check if unicode icons should be used
#[must_use]
pub fn use_unicode() -> bool {
    #[cfg(test)]
    {
        detect_use_unicode()
    }

    #[cfg(not(test))]
    {
        *USE_UNICODE_CACHE.get_or_init(detect_use_unicode)
    }
}

fn detect_use_unicode() -> bool {
    if let Ok(val) = env::var("OMG_UNICODE") {
        return val != "0" && val != "false";
    }

    colors_enabled()
}

// ═══════════════════════════════════════════════════════════════════════════
// CONDITIONAL STYLING HELPERS
// ═══════════════════════════════════════════════════════════════════════════

/// Apply color only if colors are enabled
#[inline]
#[must_use]
pub fn maybe_color(text: &str, f: impl Fn(&str) -> String) -> String {
    if colors_enabled() {
        f(text)
    } else {
        text.to_string()
    }
}

/// Get an icon (unicode or ASCII fallback)
#[inline]
#[must_use]
fn icon(unicode: &str, ascii: &str) -> String {
    if use_unicode() {
        unicode.to_string()
    } else {
        ascii.to_string()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TEXT FORMATTING
// ═══════════════════════════════════════════════════════════════════════════

/// Header with arrow prefix (e.g., "==> Installing packages")
#[must_use]
pub fn header(msg: &str) -> String {
    maybe_color(msg, |m| format!("{} {}", "==>".magenta().bold(), m.bold()))
}

/// Success message with checkmark
#[must_use]
pub fn success(msg: &str) -> String {
    maybe_color(msg, |m| format!("{} {}", icon("✓", "OK").green().bold(), m))
}

/// Error message with X
#[must_use]
pub fn error(msg: &str) -> String {
    maybe_color(msg, |m| format!("{} {}", icon("✗", "X").red().bold(), m))
}

/// Error with helpful context and suggestions
///
/// # Example
/// ```ignore
/// style::error_with_context(
///     "Package not found: rust-analyzer",
///     &["Try: omg search analyzer", "Check spelling", "Run: omg sync"]
/// );
/// ```
pub fn error_with_context(msg: &str, suggestions: &[&str]) {
    println!("{}", error(msg));
    if !suggestions.is_empty() {
        println!();
        for (i, suggestion) in suggestions.iter().enumerate() {
            println!("  {} {}", dim(&format!("{}.", i + 1)), arrow(suggestion));
        }
    }
}

/// Info message with i
#[must_use]
pub fn info(msg: &str) -> String {
    maybe_color(msg, |m| format!("{} {}", icon("ℹ", "i").blue().bold(), m))
}

/// Warning message with triangle
#[must_use]
pub fn warning(msg: &str) -> String {
    maybe_color(msg, |m| format!("{} {}", icon("⚠", "!").yellow().bold(), m))
}

/// Arrow prefix for sub-items
#[must_use]
pub fn arrow(msg: &str) -> String {
    maybe_color(msg, |m| format!("{} {}", icon("→", ">").cyan().bold(), m))
}

/// Dimmed/muted text
#[must_use]
pub fn dim(msg: &str) -> String {
    maybe_color(msg, |m| m.dimmed().to_string())
}

/// Inline code/command formatting
#[must_use]
pub fn command(cmd: &str) -> String {
    maybe_color(cmd, |c| format!("`{}`", c.cyan()))
}

/// URL formatting (underlined blue)
#[must_use]
pub fn url(link: &str) -> String {
    maybe_color(link, |l| l.underline().blue().to_string())
}

/// Package name (bold white)
#[must_use]
pub fn package(name: &str) -> String {
    maybe_color(name, |n| n.white().bold().to_string())
}

/// Version string (green)
#[must_use]
pub fn version(ver: &str) -> String {
    maybe_color(ver, |v| v.green().to_string())
}

/// Runtime name (cyan)
#[must_use]
pub fn runtime(name: &str) -> String {
    maybe_color(name, |n| n.cyan().bold().to_string())
}

/// File path (yellow)
#[must_use]
pub fn path(p: &str) -> String {
    maybe_color(p, |path| path.yellow().to_string())
}

/// Highlight important text (bold yellow)
#[must_use]
pub fn highlight(msg: &str) -> String {
    maybe_color(msg, |m| m.yellow().bold().to_string())
}

/// Size formatting (e.g., "1.5 MB")
#[must_use]
pub fn size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.2} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PROGRESS INDICATORS
// ═══════════════════════════════════════════════════════════════════════════

/// Create a spinner for indeterminate progress
#[must_use]
#[expect(clippy::expect_used, clippy::literal_string_with_formatting_args)] // Static indicatif templates are always valid; braces are template syntax not Rust format args
pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();

    // Use appropriate spinner style based on terminal capabilities
    let tick_chars = if use_unicode() {
        "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
    } else {
        "-\\|/"
    };

    let template = if colors_enabled() {
        "{spinner:.cyan} {msg}"
    } else {
        "{spinner} {msg}"
    };

    pb.set_style(
        ProgressStyle::default_spinner()
            .template(template)
            .expect("static template")
            .tick_chars(tick_chars),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use temp_env;

    #[test]
    #[serial]
    fn test_no_color_disables_colors() {
        temp_env::with_var("NO_COLOR", Some("1"), || {
            assert!(!colors_enabled());
        });
    }

    #[test]
    #[serial]
    fn test_omg_colors_always_enables() {
        temp_env::with_var("OMG_COLORS", Some("always"), || {
            assert!(colors_enabled());
        });
    }

    #[test]
    #[serial]
    fn test_omg_colors_never_disables() {
        temp_env::with_var("OMG_COLORS", Some("never"), || {
            assert!(!colors_enabled());
        });
    }

    #[test]
    #[serial]
    fn test_unicode_icons() {
        temp_env::with_var("OMG_UNICODE", Some("1"), || {
            assert_eq!(icon("✓", "OK"), "✓");
        });

        temp_env::with_var("OMG_UNICODE", Some("0"), || {
            assert_eq!(icon("✓", "OK"), "OK");
        });
    }

    #[test]
    fn test_size_formatting() {
        assert_eq!(size(500), "500 B");
        assert_eq!(size(1024), "1.0 KB");
        assert_eq!(size(1024 * 1024), "1.0 MB");
        assert_eq!(size(1024 * 1024 * 1024), "1.00 GB");
    }
}
