//! Consistent styling utilities for OMG CLI output
//!
//! All output should use these helpers for consistent UX.
//!
//! ## Features
//! - **`NO_COLOR` support**: Respects the [NO_COLOR standard](https://no-color.org/)
//! - **TTY detection**: Auto-detects terminal capabilities
//! - **Theme-aware**: Supports user-configurable color schemes
//! - **Accessibility**: WCAG AA compliant contrast ratios

use std::env;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use supports_color::Stream;

// ═══════════════════════════════════════════════════════════════════════════
// COLOR DETECTION & CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// OMG color theme configuration
#[derive(Debug, Clone, Copy)]
pub struct ColorTheme {
    pub primary: &'static str,
    pub success: &'static str,
    pub warning: &'static str,
    pub error: &'static str,
    pub info: &'static str,
    pub muted: &'static str,
}

impl Default for ColorTheme {
    fn default() -> Self {
        Self::catppuccin()
    }
}

impl ColorTheme {
    /// Catppuccin-inspired color palette (default)
    #[must_use]
    pub const fn catppuccin() -> Self {
        Self {
            primary: "86",  // Cyan
            success: "142", // Green
            warning: "221", // Yellow
            error: "203",   // Red
            info: "117",    // Blue
            muted: "245",   // Grey
        }
    }

    /// Nord theme
    #[must_use]
    pub const fn nord() -> Self {
        Self {
            primary: "109", // Nordic cyan
            success: "151", // Nordic green
            warning: "222", // Nordic yellow
            error: "167",   // Nordic red
            info: "110",    // Nordic blue
            muted: "244",   // Nordic grey
        }
    }

    /// Gruvbox theme
    #[must_use]
    pub const fn gruvbox() -> Self {
        Self {
            primary: "142", // Gruvbox aqua
            success: "142", // Gruvbox green
            warning: "214", // Gruvbox yellow
            error: "167",   // Gruvbox red
            info: "109",    // Gruvbox blue
            muted: "244",   // Gruvbox grey
        }
    }

    /// Dracula theme
    #[must_use]
    pub const fn dracula() -> Self {
        Self {
            primary: "117", // Dracula cyan
            success: "84",  // Dracula green
            warning: "228", // Dracula yellow
            error: "203",   // Dracula red
            info: "141",    // Dracula purple
            muted: "243",   // Dracula grey
        }
    }
}

use std::sync::atomic::{AtomicU8, Ordering};

const THEME_CATPPUCCIN: u8 = 0;
const THEME_NORD: u8 = 1;
const THEME_GRUVBOX: u8 = 2;
const THEME_DRACULA: u8 = 3;

static CURRENT_THEME: AtomicU8 = AtomicU8::new(THEME_CATPPUCCIN);

/// Get the current color theme
#[must_use]
pub fn theme() -> ColorTheme {
    match CURRENT_THEME.load(Ordering::Relaxed) {
        THEME_NORD => ColorTheme::nord(),
        THEME_GRUVBOX => ColorTheme::gruvbox(),
        THEME_DRACULA => ColorTheme::dracula(),
        _ => ColorTheme::catppuccin(),
    }
}

fn set_theme_id(id: u8) {
    CURRENT_THEME.store(id, Ordering::Relaxed);
}

/// Detect if colors should be enabled
///
/// Follows the [NO_COLOR standard](https://no-color.org/) and detects TTY support.
#[must_use]
pub fn colors_enabled() -> bool {
    // 1. Check NO_COLOR standard (https://no-color.org/)
    if env::var("NO_COLOR").is_ok() {
        return false;
    }

    // 2. Check if OMG_COLORS is explicitly disabled
    if let Ok(val) = env::var("OMG_COLORS") {
        if val == "never" || val == "0" || val == "false" {
            return false;
        }
        if val == "always" || val == "1" || val == "true" {
            return true;
        }
    }

    // 3. Check terminal capabilities via supports-color crate
    supports_color::on(Stream::Stdout).is_some_and(|level| level.has_basic)
}

/// Check if we're in a TTY
#[must_use]
pub fn is_tty() -> bool {
    console::user_attended()
}

/// Check if unicode icons should be used
#[must_use]
pub fn use_unicode() -> bool {
    // Check OMG_UNICODE env var
    if let Ok(val) = env::var("OMG_UNICODE") {
        return val != "0" && val != "false";
    }

    // Default to true if we have colors (likely a modern terminal)
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
pub fn icon(unicode: &str, ascii: &str) -> String {
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

/// Count/number formatting (bold)
#[must_use]
pub fn count(n: usize) -> String {
    maybe_color(&n.to_string(), |s| s.bold().to_string())
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

/// Duration formatting
#[must_use]
pub fn duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{mins}m {secs}s")
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PROGRESS INDICATORS
// ═══════════════════════════════════════════════════════════════════════════

/// Create a spinner for indeterminate progress
#[must_use]
#[allow(clippy::expect_used, clippy::literal_string_with_formatting_args)] // Static indicatif templates are always valid; braces are template syntax not Rust format args
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

/// Create a progress bar for determinate progress
#[must_use]
#[allow(clippy::expect_used, clippy::literal_string_with_formatting_args)] // Static indicatif templates are always valid; braces are template syntax not Rust format args
pub fn progress_bar(total: u64, msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);

    let template = if colors_enabled() {
        "{msg} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)"
    } else {
        "{msg} [{bar:40}] {pos}/{len} ({percent}%)"
    };

    pb.set_style(
        ProgressStyle::default_bar()
            .template(template)
            .expect("static template")
            .progress_chars(if use_unicode() { "█▓▒░" } else { "#=" }),
    );
    pb.set_message(msg.to_string());
    pb
}

/// Create a download progress bar with speed and ETA
#[must_use]
#[allow(clippy::expect_used, clippy::literal_string_with_formatting_args)] // Static indicatif templates are always valid; braces are template syntax not Rust format args
pub fn download_bar(total: u64, filename: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);

    let filename_colored = if colors_enabled() {
        filename.cyan().to_string()
    } else {
        filename.to_string()
    };

    let template = if colors_enabled() {
        "{msg}\n  [{bar:50.green/dim}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"
    } else {
        "{msg}\n  [{bar:50}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})"
    };

    pb.set_style(
        ProgressStyle::default_bar()
            .template(template)
            .expect("static template")
            .progress_chars(if use_unicode() { "━━╸" } else { "=>" }),
    );
    pb.set_message(format!("Downloading {filename_colored}"));
    pb
}

/// Create a multi-progress container for parallel operations
#[must_use]
pub fn multi_progress() -> MultiProgress {
    MultiProgress::new()
}

// ═══════════════════════════════════════════════════════════════════════════
// THEME INITIALIZATION
// ═══════════════════════════════════════════════════════════════════════════

pub fn init_theme() {
    let id = match env::var("OMG_THEME").as_deref() {
        Ok("nord") => THEME_NORD,
        Ok("gruvbox") => THEME_GRUVBOX,
        Ok("dracula") => THEME_DRACULA,
        Ok("catppuccin" | _) | Err(_) => THEME_CATPPUCCIN,
    };
    set_theme_id(id);
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

    #[test]
    fn test_duration_formatting() {
        assert_eq!(duration(500), "500ms");
        assert_eq!(duration(1500), "1.5s");
        assert_eq!(duration(65000), "1m 5s");
    }
}
