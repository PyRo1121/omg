//! Modern CLI UX inspired by bun, pnpm, and gh CLI
//!
//! Design principles:
//! - Minimal decoration, maximum information density
//! - Professional color scheme (blues, greens, subtle accents)
//! - Clear visual hierarchy without box drawing
//! - Fast, responsive feedback
//! - Context-aware status messages

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::io::IsTerminal;
use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// Rendering policy for long-running commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OutputMode {
    /// Suppress progress decoration while preserving results and errors.
    Quiet,
    /// Emit stable, non-animated status lines for redirected output.
    Plain,
    /// Render animated progress in an attended terminal.
    Interactive,
    /// Stream subprocess output live in addition to writing its log.
    Verbose,
}

static OUTPUT_MODE: AtomicU8 = AtomicU8::new(1);
static AUR_PROGRESS: OnceLock<MultiProgress> = OnceLock::new();

const fn output_mode_for(verbose: u8, quiet: bool, terminal: bool) -> OutputMode {
    if quiet {
        OutputMode::Quiet
    } else if verbose > 0 {
        OutputMode::Verbose
    } else if terminal {
        OutputMode::Interactive
    } else {
        OutputMode::Plain
    }
}

/// Configure process-wide rendering before command dispatch.
pub fn configure_output(verbose: u8, quiet: bool) {
    let mode = output_mode_for(verbose, quiet, std::io::stderr().is_terminal());
    OUTPUT_MODE.store(mode as u8, Ordering::Relaxed);
}

/// Current rendering policy.
#[must_use]
pub fn output_mode() -> OutputMode {
    match OUTPUT_MODE.load(Ordering::Relaxed) {
        0 => OutputMode::Quiet,
        2 => OutputMode::Interactive,
        3 => OutputMode::Verbose,
        _ => OutputMode::Plain,
    }
}

/// Progress display for one long-running AUR build.
pub struct AurBuildProgress {
    progress: Option<ProgressBar>,
    package: String,
    started: Instant,
    mode: OutputMode,
}

impl AurBuildProgress {
    /// Replace the live build indicator with a concise terminal result.
    pub fn finish(mut self, success: bool) {
        if let Some(progress) = self.progress.take() {
            progress.finish_and_clear();
        }

        if self.mode == OutputMode::Quiet {
            return;
        }

        let elapsed = format_duration(self.started.elapsed());
        if success {
            if crate::cli::style::colors_enabled() {
                println!(
                    "  {} {} {} {}",
                    "◆".green().bold(),
                    "forged".dimmed(),
                    self.package.cyan().bold(),
                    format!("in {elapsed}").dimmed()
                );
            } else {
                println!("  OK forged {} in {elapsed}", self.package);
            }
        } else if crate::cli::style::colors_enabled() {
            println!(
                "  {} {} {} {}",
                "◆".red().bold(),
                "forge failed".red(),
                self.package.bold(),
                format!("after {elapsed}").dimmed()
            );
        } else {
            println!("  X forge failed {} after {elapsed}", self.package);
        }
    }
}

impl Drop for AurBuildProgress {
    fn drop(&mut self) {
        if let Some(progress) = self.progress.take() {
            progress.finish_and_clear();
        }
    }
}

/// Start a compact, multi-build-safe AUR "forge" indicator.
#[must_use]
#[expect(clippy::expect_used)]
pub fn aur_build_progress(package: &str, log_path: &Path) -> AurBuildProgress {
    let mode = output_mode();
    let progress = match mode {
        OutputMode::Interactive => {
            if crate::cli::style::colors_enabled() {
                println!(
                    "  {} {}  {}",
                    "◇".magenta().bold(),
                    "AUR forge".bold(),
                    format!("log → {}", log_path.display()).dimmed()
                );
            } else {
                println!("  + AUR forge  log -> {}", log_path.display());
            }

            let progress = AUR_PROGRESS
                .get_or_init(MultiProgress::new)
                .add(ProgressBar::new_spinner());
            let template = if crate::cli::style::colors_enabled() {
                "  {spinner:.magenta.bold} {prefix:.cyan.bold}  {msg:.dim}  {elapsed_precise:.blue}"
            } else {
                "  {spinner} {prefix}  {msg}  {elapsed_precise}"
            };
            progress.set_style(
                ProgressStyle::default_spinner()
                    .template(template)
                    .expect("static AUR build template")
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
            );
            progress.set_prefix(package.to_string());
            progress.set_message("compiling source package");
            progress.enable_steady_tick(Duration::from_millis(80));
            Some(progress)
        }
        OutputMode::Verbose => {
            println!(
                "  ◆ AUR forge {} (verbose output; log: {})",
                package,
                log_path.display()
            );
            None
        }
        OutputMode::Plain => {
            println!(
                "  + Building {package} from AUR (log: {})",
                log_path.display()
            );
            None
        }
        OutputMode::Quiet => None,
    };

    AurBuildProgress {
        progress,
        package: package.to_string(),
        started: Instant::now(),
        mode,
    }
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MODERN PROGRESS INDICATORS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Modern spinner with contextual message (bun-style)
#[must_use]
#[expect(clippy::expect_used, clippy::literal_string_with_formatting_args)]
pub fn modern_spinner(phase: &str, action: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();

    // Bun-style dots spinner
    let template = if crate::cli::style::colors_enabled() {
        "{spinner:.blue.bold} {msg}"
    } else {
        "{spinner} {msg}"
    };

    pb.set_style(
        ProgressStyle::default_spinner()
            .template(template)
            .expect("static template")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );

    let msg = if crate::cli::style::colors_enabled() {
        format!("{} {}", phase.dimmed(), action)
    } else {
        format!("{phase} {action}")
    };

    pb.set_message(msg);
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Finish spinner with success
pub fn finish_success(pb: &ProgressBar, phase: &str, result: &str) {
    if crate::cli::style::colors_enabled() {
        pb.finish_with_message(format!(
            "{} {} {}",
            "✓".green().bold(),
            phase.dimmed(),
            result
        ));
    } else {
        pb.finish_with_message(format!("✓ {phase} {result}"));
    }
}

/// Finish spinner with info message
pub fn finish_info(pb: &ProgressBar, msg: &str) {
    if crate::cli::style::colors_enabled() {
        pb.finish_with_message(format!("{} {}", "·".blue(), msg.dimmed()));
    } else {
        pb.finish_with_message(format!("· {msg}"));
    }
}

/// Finish spinner and clear (for phases that don't need final status)
pub fn finish_clear(pb: &ProgressBar) {
    pb.finish_and_clear();
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// HEADERS & SECTIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Print a phase header (install, update, etc.)
pub fn print_phase_header(icon: &str, phase: &str, context: &str) {
    if crate::cli::style::colors_enabled() {
        println!("\n{} {} {}", icon, phase.bold(), context.dimmed());
    } else {
        println!("\n{icon} {phase} {context}");
    }
}

/// Print a minimal section divider
pub fn print_section(label: &str) {
    println!();
    if crate::cli::style::colors_enabled() {
        println!("  {} {}", "·".dimmed(), label.bold());
    } else {
        println!("  · {label}");
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// STATUS MESSAGES
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Success state - clean checkmark with message
pub fn print_success(msg: &str) {
    println!();
    if crate::cli::style::colors_enabled() {
        println!("  {} {}", "✓".green().bold(), msg.bold());
    } else {
        println!("  ✓ {msg}");
    }
    println!();
}

/// Success with package list
pub fn print_success_with_packages(msg: &str, packages: &[String]) {
    println!();
    if crate::cli::style::colors_enabled() {
        println!("  {} {}", "✓".green().bold(), msg.bold());
    } else {
        println!("  ✓ {msg}");
    }

    // Show up to 10 packages, then summarize
    let show_limit = 10;
    for pkg in packages.iter().take(show_limit) {
        if crate::cli::style::colors_enabled() {
            println!("    {} {}", "·".dimmed(), pkg.cyan());
        } else {
            println!("    · {pkg}");
        }
    }

    if packages.len() > show_limit {
        if crate::cli::style::colors_enabled() {
            println!(
                "    {} {} more packages",
                "·".dimmed(),
                (packages.len() - show_limit).to_string().dimmed()
            );
        } else {
            println!("    · {} more packages", packages.len() - show_limit);
        }
    }
    println!();
}

/// Error state - clean X with message
pub fn print_error(msg: &str) {
    println!();
    if crate::cli::style::colors_enabled() {
        println!("  {} {}", "✗".red().bold(), msg);
    } else {
        println!("  ✗ {msg}");
    }
    println!();
}

/// Warning state - subtle warning indicator
pub fn print_warning(msg: &str) {
    println!();
    if crate::cli::style::colors_enabled() {
        println!("  {} {}", "!".yellow().bold(), msg);
    } else {
        println!("  ! {msg}");
    }
    println!();
}

/// Info message - neutral informational
pub fn print_info(msg: &str) {
    if crate::cli::style::colors_enabled() {
        println!("  {} {}", "·".blue(), msg.dimmed());
    } else {
        println!("  · {msg}");
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PACKAGE OPERATIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// UPDATE OPERATIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Print update summary (pnpm-style)
pub fn print_update_summary(updates: &[crate::package_managers::types::UpdateInfo]) {
    println!();

    let total = updates.len();
    if crate::cli::style::colors_enabled() {
        println!(
            "  {} {} {} available",
            "·".blue(),
            total.to_string().cyan().bold(),
            if total == 1 { "update" } else { "updates" }
        );
    } else {
        println!(
            "  · {total} {} available",
            if total == 1 { "update" } else { "updates" }
        );
    }

    println!();

    // Group by source
    let official_count = updates.iter().filter(|u| u.repo != "AUR").count();
    let aur_count = updates.iter().filter(|u| u.repo == "AUR").count();

    if official_count > 0 {
        if crate::cli::style::colors_enabled() {
            println!(
                "    {} official packages",
                official_count.to_string().cyan()
            );
        } else {
            println!("    {official_count} official packages");
        }
    }

    if aur_count > 0 {
        if crate::cli::style::colors_enabled() {
            println!("    {} AUR packages", aur_count.to_string().magenta());
        } else {
            println!("    {aur_count} AUR packages");
        }
    }

    println!();

    // Show sample of updates (up to 15)
    let display_limit = 15;
    for update in updates.iter().take(display_limit) {
        if crate::cli::style::colors_enabled() {
            let repo_badge = if update.repo == "AUR" {
                "aur".magenta().to_string()
            } else {
                update.repo.as_str().dimmed().to_string()
            };

            println!(
                "    {} {} {} {}",
                update.name.cyan(),
                update.old_version.dimmed(),
                "→".dimmed(),
                update.new_version.green()
            );
            println!("      {repo_badge}");
        } else {
            println!(
                "    {} {} → {} ({})",
                update.name, update.old_version, update.new_version, update.repo
            );
        }
    }

    if updates.len() > display_limit {
        println!();
        if crate::cli::style::colors_enabled() {
            println!(
                "    {} and {} more",
                "·".dimmed(),
                (updates.len() - display_limit).to_string().dimmed()
            );
        } else {
            println!("    · and {} more", updates.len() - display_limit);
        }
    }

    println!();
}

/// Print "up to date" status
pub fn print_up_to_date() {
    println!();
    if crate::cli::style::colors_enabled() {
        println!("  {} System is up to date", "✓".green().bold());
    } else {
        println!("  ✓ System is up to date");
    }
    println!();
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// AUR-SPECIFIC
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Print AUR package info with security notice
pub fn print_aur_package_info(name: &str, version: &str, description: &str) {
    println!();

    if crate::cli::style::colors_enabled() {
        println!("  {} Package from AUR", "·".magenta());
        println!();
        println!("    {} {}", "name".dimmed(), name.cyan().bold());
        println!("    {} {}", "version".dimmed(), version.green());
        if !description.is_empty() {
            println!("    {} {}", "description".dimmed(), description);
        }
    } else {
        println!("  · Package from AUR");
        println!();
        println!("    name {name}");
        println!("    version {version}");
        if !description.is_empty() {
            println!("    description {description}");
        }
    }

    println!();

    // Security notice - subdued but present
    if crate::cli::style::colors_enabled() {
        println!("  {} User-submitted package", "!".yellow().bold());
        println!("    {} Not vetted by Arch maintainers", "·".dimmed());
        println!("    {} Review PKGBUILD before installing", "·".dimmed());
    } else {
        println!("  ! User-submitted package");
        println!("    · Not vetted by Arch maintainers");
        println!("    · Review PKGBUILD before installing");
    }

    println!();
}

/// Print AUR build progress phase
pub fn print_aur_build_phase(phase: &str, package: &str) {
    if crate::cli::style::colors_enabled() {
        println!("  {} {} {}", "·".magenta(), phase.dimmed(), package.cyan());
    } else {
        println!("  · {phase} {package}");
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// TIMING & STATS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_policy_prioritizes_quiet_and_verbose() {
        assert_eq!(output_mode_for(0, true, true), OutputMode::Quiet);
        assert_eq!(output_mode_for(1, false, true), OutputMode::Verbose);
        assert_eq!(output_mode_for(0, false, true), OutputMode::Interactive);
        assert_eq!(output_mode_for(0, false, false), OutputMode::Plain);
    }

    #[test]
    fn elapsed_time_stays_compact() {
        assert_eq!(format_duration(Duration::from_secs(9)), "9s");
        assert_eq!(format_duration(Duration::from_secs(125)), "2m 05s");
    }
}
