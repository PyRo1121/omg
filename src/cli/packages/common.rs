//! Common utilities for package operations
//!
//! Also hosts behavior shared verbatim between compiled package-manager
//! backends (cold-path updates, service-backed removal, daemon update
//! probes) so each backend file only carries what genuinely differs.

use anyhow::Result;

#[cfg(unix)]
use crate::core::client::DaemonClient;
use crate::package_managers::types::UpdateInfo;

/// Get description truncation width based on terminal size.
///
/// Reserves space for package name, version, source label, and formatting
/// chrome (~45 chars), then uses the rest for the description.
/// Falls back to 50 chars if terminal width is unavailable.
pub fn description_width() -> usize {
    crossterm::terminal::size()
        .map(|(cols, _)| {
            let cols = cols as usize;
            // Reserve ~45 chars for "  name version (source) - " prefix chrome
            cols.saturating_sub(45).max(20)
        })
        .unwrap_or(50)
}

/// Validate a search query for safety.
///
/// Rejects queries that are too long, contain control characters,
/// path traversal sequences, or shell metacharacters.
pub fn validate_search_query(query: &str) -> Result<()> {
    if query.len() > 100 {
        anyhow::bail!("Search query too long (max 100 characters)");
    }
    if query.chars().any(char::is_control) {
        anyhow::bail!("Search query contains invalid characters");
    }
    if query.contains('/') || query.contains('\\') || query.contains("..") {
        anyhow::bail!("Invalid search query: path traversal detected");
    }
    if query.chars().any(|c| ";|&><$".contains(c)) {
        anyhow::bail!("Invalid search query: shell metacharacters detected");
    }
    Ok(())
}

/// Check if a search query passes validation (returns false on failure).
///
/// Convenience wrapper for sync code paths that return `bool` instead of `Result`.
#[must_use]
pub fn is_valid_search_query(query: &str) -> bool {
    validate_search_query(query).is_ok()
}

/// Probe the daemon for pending official-repository updates.
///
/// Returns `None` when the daemon is unavailable or reports an error so the
/// caller can fall back to querying the package manager directly.
#[cfg(unix)]
pub(crate) async fn try_daemon_list_updates() -> Option<Vec<UpdateInfo>> {
    let mut client = DaemonClient::connect().await.ok()?;
    let entries = client.list_updates().await.ok()?;
    Some(
        entries
            .into_iter()
            .map(|e| UpdateInfo {
                name: e.name,
                old_version: e.old_version,
                new_version: e.new_version,
                repo: e.repo,
            })
            .collect(),
    )
}

/// Non-Unix stub preserving the asynchronous command interface.
#[cfg(not(unix))]
#[allow(
    clippy::unused_async,
    reason = "the non-Unix implementation preserves the asynchronous command interface"
)]
pub(crate) async fn try_daemon_list_updates() -> Option<Vec<UpdateInfo>> {
    None
}

/// Shared cold-path update flow for the Debian and generic backends:
/// optionally sync, list official updates, confirm, and upgrade.
///
/// These backends have no AUR equivalent, so `check_only`, `dry_run`, and the
/// confirmation prompt cover the entire decision tree.
///
/// Compiled exactly when a consumer backend exists: the Debian module (under
/// `debian`/`debian-pure`) or the generic module (when Arch is absent).
#[cfg(any(feature = "debian", feature = "debian-pure", not(feature = "arch")))]
pub(crate) async fn update_official_only(check_only: bool, yes: bool, dry_run: bool) -> Result<()> {
    use owo_colors::OwoColorize;

    let start_time = std::time::Instant::now();
    let pm = crate::package_managers::get_package_manager()?;
    let skip_sync = check_only || dry_run;

    if check_only || dry_run {
        crate::cli::modern_ui::print_phase_header(
            "🔄",
            "Update",
            if check_only {
                "Checking for updates (no sync)"
            } else {
                "Dry run - checking for updates"
            },
        );
    } else {
        crate::cli::modern_ui::print_phase_header("🔄", "Update", "Checking for updates");
        if !skip_sync {
            let pb = crate::cli::modern_ui::modern_spinner("Syncing", "package databases");
            let sync_start = std::time::Instant::now();
            pm.sync().await?;
            crate::cli::modern_ui::finish_success(
                &pb,
                "Synced",
                &format!("in {:.2}s", sync_start.elapsed().as_secs_f64()),
            );
        }
    }

    let pb = crate::cli::modern_ui::modern_spinner("Checking", "official repositories");
    let check_start = std::time::Instant::now();
    let official_updates = match try_daemon_list_updates().await {
        Some(updates) => updates,
        None => pm.list_updates().await?,
    };
    if official_updates.is_empty() {
        crate::cli::modern_ui::finish_info(&pb, "No updates in official repositories");
    } else {
        crate::cli::modern_ui::finish_success(
            &pb,
            "Found",
            &format!(
                "{} update(s) in {:.2}s",
                official_updates.len(),
                check_start.elapsed().as_secs_f64()
            ),
        );
    }

    println!();
    if official_updates.is_empty() {
        crate::cli::modern_ui::print_up_to_date();
        return Ok(());
    }

    if dry_run {
        return update_official_only_dry_run(&official_updates);
    }

    crate::cli::modern_ui::print_update_summary(&official_updates);

    if check_only {
        println!();
        println!(
            "  {} Run {} to install updates",
            "→".dimmed(),
            "omg update".cyan().bold()
        );
        println!();
        return Ok(());
    }

    if !yes && console::user_attended() {
        println!();
        if !confirm_proceed_with_upgrade().await? {
            println!();
            println!("  {} Upgrade cancelled", "✗".yellow());
            println!();
            return Ok(());
        }
    } else if !yes {
        anyhow::bail!("Use --yes for non-interactive updates");
    }

    println!();
    crate::cli::modern_ui::print_section("Installing updates");

    let count = official_updates.len();
    let pb =
        crate::cli::modern_ui::modern_spinner("Upgrading", &format!("{count} official packages"));
    pm.update().await?;
    crate::cli::modern_ui::finish_success(&pb, "Upgraded", &format!("{count} packages"));

    let duration_ms = start_time.elapsed().as_millis() as u64;
    crate::core::usage::track_update_timed(count, duration_ms, true, None);
    crate::cli::modern_ui::print_success(&format!("Upgraded {count} packages"));
    Ok(())
}

/// Blocking terminal prompt moved off the async executor thread.
#[cfg(any(feature = "debian", feature = "debian-pure", not(feature = "arch")))]
async fn confirm_proceed_with_upgrade() -> Result<bool> {
    Ok(tokio::task::spawn_blocking(|| {
        dialoguer::Confirm::with_theme(&crate::cli::ui::prompt_theme())
            .with_prompt("Proceed with upgrade?")
            .default(true)
            .interact()
    })
    .await
    .map_err(|error| anyhow::anyhow!("Confirmation prompt task failed: {error}"))??)
}

/// Dry-run preview shared by the Debian and generic backends.
///
/// Download sizes are unknown without resolving dependencies, so none are
/// claimed.
/// Compiled exactly when a consumer backend exists: the Debian module (under
/// `debian`/`debian-pure`) or the generic module (when Arch is absent).
#[cfg(any(feature = "debian", feature = "debian-pure", not(feature = "arch")))]
pub(crate) fn update_official_only_dry_run(updates: &[UpdateInfo]) -> Result<()> {
    let names: Vec<String> = updates.iter().map(|update| update.name.clone()).collect();
    crate::core::security::validate_package_names(&names)?;

    crate::cli::ui::print_header("OMG", "Dry Run - Update Preview");
    crate::cli::ui::print_spacer();
    println!(
        "  {} The following packages would be updated:\n",
        crate::cli::style::info("→")
    );

    for update in updates.iter().take(50) {
        println!(
            "    {} {} {} {} {} ({})",
            crate::cli::style::success("↑"),
            crate::cli::style::package(&update.name),
            crate::cli::style::dim(&update.old_version),
            crate::cli::style::arrow("->"),
            crate::cli::style::version(&update.new_version),
            crate::cli::style::dim("unknown")
        );
    }

    if updates.len() > 50 {
        println!(
            "    {}",
            crate::cli::style::dim(&format!("(+{} more updates)", updates.len() - 50))
        );
    }

    crate::cli::ui::print_spacer();
    println!(
        "  {} Total updates: {}",
        crate::cli::style::info("→"),
        updates.len()
    );
    println!(
        "  {} Estimated download: unknown",
        crate::cli::style::info("→")
    );
    crate::cli::ui::print_dry_run_footer();
    Ok(())
}

/// Removal orchestration shared by every compiled backend: timing, history,
/// usage tracking, and success reporting around `PackageService::remove`.
pub(crate) async fn remove_via_service(packages: &[String]) -> Result<()> {
    use crate::core::packages::PackageService;

    let start_time = std::time::Instant::now();
    let pm = crate::package_managers::get_package_manager()?;
    let service = PackageService::new(pm)?;

    crate::cli::modern_ui::print_phase_header(
        "🗑️",
        "Remove",
        &format!("{} package(s)", packages.len()),
    );
    println!();

    // PackageService::remove currently ignores its recursion flag (the Arch
    // backend always cleans unneeded dependencies, other backends never do);
    // pass `false` and let the backend defaults decide.
    let result = service.remove(packages, false).await;

    let duration_ms = start_time.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => crate::core::usage::track_remove_timed(packages, duration_ms, true, None),
        Err(e) => crate::core::usage::track_remove_timed(
            packages,
            duration_ms,
            false,
            Some(&e.to_string()),
        ),
    }

    result?;

    crate::cli::ui::print_spacer();
    crate::cli::ui::print_success("Packages removed successfully");
    crate::cli::ui::print_spacer();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(feature = "debian", feature = "debian-pure", not(feature = "arch")))]
    fn dry_run_preview_reports_unknown_download_size() {
        // The preview must never fabricate download sizes it did not resolve.
        let updates = vec![crate::package_managers::types::UpdateInfo {
            name: "example".to_string(),
            old_version: "1.0".to_string(),
            new_version: "2.0".to_string(),
            repo: "core".to_string(),
        }];
        assert!(update_official_only_dry_run(&updates).is_ok());
    }

    #[test]
    fn test_validate_search_query_valid() {
        assert!(validate_search_query("firefox").is_ok());
        assert!(validate_search_query("lib32-mesa").is_ok());
        assert!(validate_search_query("python-numpy").is_ok());
    }

    #[test]
    fn test_validate_search_query_too_long() {
        let long = "a".repeat(101);
        let err = validate_search_query(&long).unwrap_err();
        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn test_validate_search_query_control_chars() {
        let err = validate_search_query("test\x00query").unwrap_err();
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn test_validate_search_query_path_traversal() {
        let err = validate_search_query("../etc/passwd").unwrap_err();
        assert!(err.to_string().contains("path traversal"));
    }

    #[test]
    fn test_validate_search_query_shell_metacharacters() {
        let err = validate_search_query("test;rm -rf").unwrap_err();
        assert!(err.to_string().contains("shell metacharacters"));
    }

    #[test]
    fn test_is_valid_search_query() {
        assert!(is_valid_search_query("firefox"));
        assert!(!is_valid_search_query("../passwd"));
        assert!(!is_valid_search_query("test;ls"));
    }

    #[test]
    fn test_description_width_has_minimum() {
        assert!(description_width() >= 20);
    }
}
