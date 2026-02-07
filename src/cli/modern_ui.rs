//! Modern CLI UX inspired by bun, pnpm, and gh CLI
//!
//! Design principles:
//! - Minimal decoration, maximum information density
//! - Professional color scheme (blues, greens, subtle accents)
//! - Clear visual hierarchy without box drawing
//! - Fast, responsive feedback
//! - Context-aware status messages

use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::time::Duration;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// MODERN PROGRESS INDICATORS
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Modern spinner with contextual message (bun-style)
#[must_use]
#[allow(clippy::expect_used, clippy::literal_string_with_formatting_args)]
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

/// Show package being operated on
pub fn print_package_action(action: &str, package: &str, version: Option<&str>) {
    if crate::cli::style::colors_enabled() {
        if let Some(ver) = version {
            println!(
                "  {} {} {}",
                action.dimmed(),
                package.cyan().bold(),
                ver.dimmed()
            );
        } else {
            println!("  {} {}", action.dimmed(), package.cyan().bold());
        }
    } else if let Some(ver) = version {
        println!("  {action} {package} {ver}");
    } else {
        println!("  {action} {package}");
    }
}

/// Show a step in multi-step process (minimal)
pub fn print_step(msg: &str) {
    if crate::cli::style::colors_enabled() {
        println!("    {} {}", "→".dimmed(), msg.dimmed());
    } else {
        println!("    → {msg}");
    }
}

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

/// Print operation timing
pub fn print_timing(label: &str, duration: std::time::Duration) {
    let seconds = duration.as_secs_f64();

    if crate::cli::style::colors_enabled() {
        if seconds < 1.0 {
            println!(
                "  {} {} in {}",
                "✓".green(),
                label.dimmed(),
                format!("{:.0}ms", seconds * 1000.0).cyan()
            );
        } else {
            println!(
                "  {} {} in {}",
                "✓".green(),
                label.dimmed(),
                format!("{seconds:.2}s").cyan()
            );
        }
    } else if seconds < 1.0 {
        println!("  ✓ {label} in {:.0}ms", seconds * 1000.0);
    } else {
        println!("  ✓ {label} in {seconds:.2}s");
    }
}

/// Print download progress (minimal, updates in-place)
#[must_use]
#[allow(clippy::expect_used, clippy::literal_string_with_formatting_args)]
pub fn download_progress(filename: &str, total_bytes: u64) -> ProgressBar {
    let pb = ProgressBar::new(total_bytes);

    let template = if crate::cli::style::colors_enabled() {
        "  {msg} [{bar:30.cyan/blue}] {bytes}/{total_bytes} {bytes_per_sec}"
    } else {
        "  {msg} [{bar:30}] {bytes}/{total_bytes} {bytes_per_sec}"
    };

    pb.set_style(
        ProgressStyle::default_bar()
            .template(template)
            .expect("static template")
            .progress_chars("━━╸"),
    );

    if crate::cli::style::colors_enabled() {
        pb.set_message(format!("{}", filename.dimmed()));
    } else {
        pb.set_message(filename.to_string());
    }

    pb
}
