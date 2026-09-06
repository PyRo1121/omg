//! Modern CLI UX inspired by bun, pnpm, and gh CLI
//!
//! Design principles:
//! - Minimal decoration, maximum information density
//! - Professional color scheme (blues, greens, subtle accents)
//! - Clear visual hierarchy without box drawing
//! - Fast, responsive feedback
//! - Context-aware status messages

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use owo_colors::OwoColorize;
use std::io::IsTerminal;
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
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
/// Process-wide registry for every live progress lane. Attaching lanes here
/// (instead of leaving them standalone) lets [`quiesce_terminal`] hide the
/// whole family at once when an interactive prompt such as a PKGBUILD review
/// or an ALPM dialog must own the terminal.
static PROGRESS_REGISTRY: OnceLock<MultiProgress> = OnceLock::new();
/// Refcount of active [`TerminalQuiesceGuard`]s; transitions run under the
/// lock so the draw target flips exactly once per 0->1 and N->0 boundary.
static QUIESCE_COUNT: Mutex<usize> = Mutex::new(0);
/// Final lines rendered while quiesced, replayed in FIFO order when the last
/// guard drops.
static DEFERRED_LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn lock_quiesce_count() -> MutexGuard<'static, usize> {
    QUIESCE_COUNT.lock().unwrap_or_else(PoisonError::into_inner)
}

fn lock_deferred_lines() -> MutexGuard<'static, Vec<String>> {
    DEFERRED_LINES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

fn quiesce_active() -> bool {
    *lock_quiesce_count() > 0
}

fn hide_progress(progress: &MultiProgress) {
    let _ = progress.clear();
    progress.set_draw_target(ProgressDrawTarget::hidden());
}

/// Suppress live progress drawing while an interactive prompt owns the
/// terminal.
///
/// Final result lines raised during the quiesce are buffered and printed once
/// the last guard drops, so multi-line output cannot be overwritten by a
/// redrawn spinner.
#[must_use]
pub fn quiesce_terminal() -> TerminalQuiesceGuard {
    let mut count = lock_quiesce_count();
    *count += 1;
    if *count == 1 {
        hide_progress(progress_registry());
    }
    TerminalQuiesceGuard { _private: () }
}

/// RAII handle from [`quiesce_terminal`]; restores progress drawing and
/// flushes deferred lines when the final guard drops.
pub struct TerminalQuiesceGuard {
    _private: (),
}

impl Drop for TerminalQuiesceGuard {
    fn drop(&mut self) {
        let mut count = lock_quiesce_count();
        *count -= 1;
        if *count == 0 {
            progress_registry().set_draw_target(ProgressDrawTarget::stderr());
            let mut deferred = lock_deferred_lines();
            for line in deferred.drain(..) {
                println!("{line}");
            }
        }
    }
}

fn progress_registry() -> &'static MultiProgress {
    PROGRESS_REGISTRY.get_or_init(MultiProgress::new)
}

/// Attach a lane to the process-wide progress registry so quiescing can hide
/// it together with every other live lane.
///
/// Lanes are visible only in [`OutputMode::Interactive`]; every other mode
/// receives an invisible bar so callers drive one code path regardless of
/// policy while redirected or quiet output stays free of animation and ANSI.
pub(crate) fn register_spinner(bar: ProgressBar) -> ProgressBar {
    if output_mode() != OutputMode::Interactive {
        return ProgressBar::hidden();
    }
    progress_registry().add(bar)
}

/// Print `line`, or buffer it while a terminal quiesce is active.
pub(crate) fn emit_or_defer(line: String) {
    if quiesce_active() {
        lock_deferred_lines().push(line);
    } else {
        println!("{line}");
    }
}

/// Buffer `line` for replay when the terminal quiesce ends.
fn defer_line(line: String) {
    lock_deferred_lines().push(line);
}

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

    let colors_enabled = crate::cli::style::colors_enabled();
    console::set_colors_enabled(colors_enabled);
    console::set_colors_enabled_stderr(colors_enabled);
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
        let line = if success {
            if crate::cli::style::colors_enabled() {
                format!(
                    "  {} {} {} {}",
                    "◆".green().bold(),
                    "forged".dimmed(),
                    self.package.cyan().bold(),
                    format!("in {elapsed}").dimmed()
                )
            } else {
                format!("  OK forged {} in {elapsed}", self.package)
            }
        } else if crate::cli::style::colors_enabled() {
            format!(
                "  {} {} {} {}",
                "◆".red().bold(),
                "forge failed".red(),
                self.package.bold(),
                format!("after {elapsed}").dimmed()
            )
        } else {
            format!("  X forge failed {} after {elapsed}", self.package)
        };
        emit_or_defer(line);
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
    // Package names reach lane prefixes rendered raw onto the terminal.
    let package = crate::cli::style::sanitize_terminal_text(package);
    let package = package.as_str();
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

            let progress = register_spinner(ProgressBar::new_spinner());
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

pub(crate) fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3_600 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        let hours = seconds / 3_600;
        let minutes = (seconds % 3_600) / 60;
        format!("{hours}h {minutes:02}m {:02}s", seconds % 60)
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MODERN PROGRESS INDICATORS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Modern spinner with contextual message (bun-style)
#[must_use]
#[expect(clippy::expect_used, clippy::literal_string_with_formatting_args)]
pub fn modern_spinner(phase: &str, action: &str) -> ProgressBar {
    if output_mode() == OutputMode::Quiet {
        return ProgressBar::hidden();
    }

    let pb = register_spinner(ProgressBar::new_spinner());

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
    let message = if crate::cli::style::colors_enabled() {
        format!("{} {} {}", "✓".green().bold(), phase.dimmed(), result)
    } else {
        format!("✓ {phase} {result}")
    };
    // Animation is transient terminal state. Clear it before writing the
    // completed status as a durable line; a visible finished bar is redrawn
    // whenever a prompt or subprocess suspends the shared MultiProgress.
    pb.finish_and_clear();
    if output_mode() != OutputMode::Quiet {
        emit_or_defer(message);
    }
}

/// Finish spinner with info message
pub fn finish_info(pb: &ProgressBar, msg: &str) {
    let message = if crate::cli::style::colors_enabled() {
        format!("{} {}", "·".blue(), msg.dimmed())
    } else {
        format!("· {msg}")
    };
    pb.finish_and_clear();
    if output_mode() != OutputMode::Quiet {
        emit_or_defer(message);
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
    if output_mode() == OutputMode::Quiet {
        return;
    }
    let line = if crate::cli::style::colors_enabled() {
        format!("\n{} {} {}", icon, phase.bold(), context.dimmed())
    } else {
        format!("\n{icon} {phase} {context}")
    };
    emit_or_defer(line);
}

/// Print a minimal section divider
pub fn print_section(label: &str) {
    if output_mode() == OutputMode::Quiet {
        return;
    }
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
    if output_mode() == OutputMode::Quiet {
        return;
    }
    let line = if crate::cli::style::colors_enabled() {
        format!("  {} {}", "✓".green().bold(), msg.bold())
    } else {
        format!("  ✓ {msg}")
    };
    if quiesce_active() {
        defer_line(line);
        return;
    }
    println!();
    println!("{line}");
    println!();
}

/// Success with package list
pub fn print_success_with_packages(msg: &str, packages: &[String]) {
    if output_mode() == OutputMode::Quiet {
        return;
    }
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
    let line = if crate::cli::style::colors_enabled() {
        format!("\n  {} {}\n", "✗".red().bold(), msg)
    } else {
        format!("\n  ✗ {msg}\n")
    };
    emit_or_defer(line);
}

/// Warning state - subtle warning indicator
pub fn print_warning(msg: &str) {
    let line = if crate::cli::style::colors_enabled() {
        format!("  {} {}", "!".yellow().bold(), msg)
    } else {
        format!("  ! {msg}")
    };
    if quiesce_active() {
        defer_line(line);
        return;
    }
    println!();
    println!("{line}");
    println!();
}

/// Info message - neutral informational
pub fn print_info(msg: &str) {
    if output_mode() == OutputMode::Quiet {
        return;
    }
    let line = if crate::cli::style::colors_enabled() {
        format!("  {} {}", "·".blue(), msg.dimmed())
    } else {
        format!("  · {msg}")
    };
    emit_or_defer(line);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PACKAGE OPERATIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// UPDATE OPERATIONS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Print update summary (pnpm-style)
pub fn print_update_summary(updates: &[crate::package_managers::types::UpdateInfo]) {
    if output_mode() == OutputMode::Quiet {
        return;
    }
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

    // Show sample of updates (up to 15), one line each with the source
    // badge inline so long update lists stay scannable.
    let display_limit = 15;
    for update in updates.iter().take(display_limit) {
        let name = crate::cli::style::sanitize_terminal_text(&update.name);
        let old_version = crate::cli::style::sanitize_terminal_text(&update.old_version);
        let new_version = crate::cli::style::sanitize_terminal_text(&update.new_version);
        let repo = crate::cli::style::sanitize_terminal_text(&update.repo);

        if crate::cli::style::colors_enabled() {
            let repo_badge = if update.repo == "AUR" {
                "aur".magenta().to_string()
            } else {
                repo.as_str().dimmed().to_string()
            };

            println!(
                "    {} {} {} {} {}",
                name.cyan(),
                old_version.dimmed(),
                "→".dimmed(),
                new_version.green(),
                repo_badge
            );
        } else {
            println!("    {name} {old_version} → {new_version} ({repo})");
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
    if output_mode() == OutputMode::Quiet {
        return;
    }
    // AUR strings are attacker-controlled: a malicious description can carry
    // terminal escape sequences, so every field is sanitized before display.
    let name = crate::cli::style::sanitize_terminal_text(name);
    let version = crate::cli::style::sanitize_terminal_text(version);
    let description = crate::cli::style::sanitize_terminal_text(description);
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
    if output_mode() == OutputMode::Quiet {
        return;
    }
    let package = crate::cli::style::sanitize_terminal_text(package);
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
    use indicatif::{ProgressDrawTarget, ProgressStyle, TermLike};
    use std::io;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug, Default)]
    struct RecordingTerm {
        events: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingTerm {
        fn record(&self, event: impl Into<String>) {
            self.events
                .lock()
                .expect("recording terminal lock")
                .push(event.into());
        }

        fn events(&self) -> Vec<String> {
            self.events.lock().expect("recording terminal lock").clone()
        }
    }

    impl TermLike for RecordingTerm {
        fn width(&self) -> u16 {
            120
        }

        fn move_cursor_up(&self, lines: usize) -> io::Result<()> {
            self.record(format!("up:{lines}"));
            Ok(())
        }

        fn move_cursor_down(&self, lines: usize) -> io::Result<()> {
            self.record(format!("down:{lines}"));
            Ok(())
        }

        fn move_cursor_right(&self, columns: usize) -> io::Result<()> {
            self.record(format!("right:{columns}"));
            Ok(())
        }

        fn move_cursor_left(&self, columns: usize) -> io::Result<()> {
            self.record(format!("left:{columns}"));
            Ok(())
        }

        fn write_line(&self, text: &str) -> io::Result<()> {
            self.record(format!("line:{text}"));
            Ok(())
        }

        fn write_str(&self, text: &str) -> io::Result<()> {
            self.record(format!("write:{text}"));
            Ok(())
        }

        fn clear_line(&self) -> io::Result<()> {
            self.record("clear");
            Ok(())
        }

        fn flush(&self) -> io::Result<()> {
            self.record("flush");
            Ok(())
        }
    }

    fn recording_spinner(message: &str) -> (ProgressBar, RecordingTerm) {
        configure_output(0, false);
        let terminal = RecordingTerm::default();
        let progress = ProgressBar::with_draw_target(
            None,
            ProgressDrawTarget::term_like(Box::new(terminal.clone())),
        );
        progress.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner} {msg}")
                .expect("static progress template"),
        );
        progress.set_message(message.to_string());
        progress.tick();
        (progress, terminal)
    }

    #[test]
    fn hiding_progress_clears_existing_rows_before_a_prompt() {
        let terminal = RecordingTerm::default();
        let progress = MultiProgress::with_draw_target(ProgressDrawTarget::term_like(Box::new(
            terminal.clone(),
        )));
        let spinner = progress.add(ProgressBar::new_spinner());
        spinner.set_message("active build");
        spinner.tick();
        let event_count = terminal.events().len();

        hide_progress(&progress);

        assert!(
            terminal.events()[event_count..]
                .iter()
                .any(|event| event == "clear"),
            "an existing progress row must be erased before prompt rendering"
        );
    }

    #[test]
    #[serial_test::serial]
    fn finished_spinner_clears_its_animated_lane() {
        let (progress, terminal) = recording_spinner("Checking package sources");

        finish_success(&progress, "Checked", "package sources");

        let events = terminal.events();
        assert!(
            !events
                .iter()
                .any(|event| event.contains("Checked package sources")),
            "completed status must be durable output, not a retained animation lane: {events:?}"
        );
    }

    #[test]
    #[serial_test::serial]
    fn finished_info_spinner_clears_its_animated_lane() {
        let (progress, terminal) = recording_spinner("Checking official repositories");

        finish_info(&progress, "No updates in official repositories");

        let events = terminal.events();
        assert!(
            !events.iter().any(|event| event.contains("No updates")),
            "completed status must be durable output, not a retained animation lane: {events:?}"
        );
    }

    #[test]
    #[serial_test::serial]
    fn quiet_spinners_are_hidden() {
        configure_output(0, true);
        let progress = modern_spinner("phase", "action");
        assert!(progress.is_hidden());
        configure_output(0, false);
    }

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
        assert_eq!(format_duration(Duration::from_secs(3_725)), "1h 02m 05s");
    }
}
