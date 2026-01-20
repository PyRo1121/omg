//! Consistent styling utilities for OMG CLI output
//!
//! All output should use these helpers for consistent UX.

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;

// ═══════════════════════════════════════════════════════════════════════════
// TEXT FORMATTING
// ═══════════════════════════════════════════════════════════════════════════

/// Header with arrow prefix (e.g., "==> Installing packages")
#[must_use]
pub fn header(msg: &str) -> String {
    format!("{} {}", "==>".magenta().bold(), msg.bold())
}

/// Success message with checkmark
#[must_use]
pub fn success(msg: &str) -> String {
    format!("{} {}", "✓".green().bold(), msg)
}

/// Error message with X
#[must_use]
pub fn error(msg: &str) -> String {
    format!("{} {}", "✗".red().bold(), msg)
}

/// Info message with i
#[must_use]
pub fn info(msg: &str) -> String {
    format!("{} {}", "ℹ".blue().bold(), msg)
}

/// Warning message with triangle
#[must_use]
pub fn warning(msg: &str) -> String {
    format!("{} {}", "⚠".yellow().bold(), msg)
}

/// Arrow prefix for sub-items
#[must_use]
pub fn arrow(msg: &str) -> String {
    format!("{} {}", "→".cyan().bold(), msg)
}

/// Dimmed/muted text
#[must_use]
pub fn dim(msg: &str) -> String {
    msg.dimmed().to_string()
}

/// Inline code/command formatting
#[must_use]
pub fn command(cmd: &str) -> String {
    format!("`{}`", cmd.cyan())
}

/// URL formatting (underlined blue)
#[must_use]
pub fn url(link: &str) -> String {
    link.underline().blue().to_string()
}

/// Package name (bold white)
#[must_use]
pub fn package(name: &str) -> String {
    name.white().bold().to_string()
}

/// Version string (green)
#[must_use]
pub fn version(ver: &str) -> String {
    ver.green().to_string()
}

/// Runtime name (cyan)
#[must_use]
pub fn runtime(name: &str) -> String {
    name.cyan().bold().to_string()
}

/// File path (yellow)
#[must_use]
pub fn path(p: &str) -> String {
    p.yellow().to_string()
}

/// Highlight important text (bold yellow)
#[must_use]
pub fn highlight(msg: &str) -> String {
    msg.yellow().bold().to_string()
}

/// Count/number formatting (bold)
#[must_use]
pub fn count(n: usize) -> String {
    n.to_string().bold().to_string()
}

/// Size formatting (e.g., "1.5 MB")
#[must_use]
pub fn size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
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
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{}m {}s", mins, secs)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PROGRESS INDICATORS
// ═══════════════════════════════════════════════════════════════════════════

/// Create a spinner for indeterminate progress
#[must_use]
#[allow(clippy::literal_string_with_formatting_args, clippy::expect_used)]
pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .expect("static template")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

/// Create a progress bar for determinate progress
#[must_use]
#[allow(clippy::expect_used)]
pub fn progress_bar(total: u64, msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)")
            .expect("static template")
            .progress_chars("█▓▒░"),
    );
    pb.set_message(msg.to_string());
    pb
}

/// Create a download progress bar with speed and ETA
#[must_use]
#[allow(clippy::expect_used)]
pub fn download_bar(total: u64, filename: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg}\n  [{bar:50.green/dim}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .expect("static template")
            .progress_chars("━━╸"),
    );
    pb.set_message(format!("Downloading {}", filename.cyan()));
    pb
}

/// Create a multi-progress container for parallel operations
#[must_use]
pub fn multi_progress() -> MultiProgress {
    MultiProgress::new()
}

// ═══════════════════════════════════════════════════════════════════════════
// FORMATTED OUTPUT HELPERS
// ═══════════════════════════════════════════════════════════════════════════

/// Print a key-value pair with consistent formatting
pub fn print_kv(key: &str, value: &str) {
    println!("  {}: {}", key.bold(), value);
}

/// Print a list item with bullet
pub fn print_bullet(item: &str) {
    println!("  {} {}", "•".cyan(), item);
}

/// Print a numbered list item
pub fn print_numbered(n: usize, item: &str) {
    println!("  {}. {}", n.to_string().bold(), item);
}

/// Print a section header
pub fn print_section(title: &str) {
    println!("\n{}\n", title.bold().underline());
}

/// Print a horizontal rule
pub fn print_hr() {
    println!("{}", "─".repeat(60).dimmed());
}

/// Print a blank line
pub fn print_blank() {
    println!();
}
