use std::io::Write;
use std::time::Instant;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::cli::packages::common::{description_width, validate_search_query};
use crate::cli::style;
use crate::core::Package;
use crate::package_managers::get_package_manager;

#[cfg(unix)]
use crate::core::client::{DaemonClient, PooledSyncClient};

#[cfg(feature = "arch")]
use crate::package_managers::AurClient;

#[derive(Serialize)]
struct DisplayPackage {
    name: String,
    version: String,
    description: String,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    votes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    popularity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maintainer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    out_of_date: Option<bool>,
}

impl DisplayPackage {
    #[allow(clippy::implicit_clone)] // Version type varies by feature flag
    fn from_package(p: Package) -> Self {
        Self {
            name: p.name,
            version: p.version.to_string(),
            description: p.description,
            source: p.source.to_string(),
            votes: None,
            popularity: None,
            maintainer: None,
            out_of_date: None,
        }
    }

    #[cfg(feature = "arch")]
    fn from_aur_detail(p: crate::package_managers::AurPackageDetail) -> Self {
        Self {
            name: p.name,
            version: p.version,
            description: p.description.unwrap_or_default(),
            source: "AUR".to_string(),
            votes: Some(p.num_votes),
            popularity: Some(p.popularity),
            maintainer: p.maintainer.clone(),
            out_of_date: Some(p.out_of_date.is_some()),
        }
    }
}

#[allow(clippy::fn_params_excessive_bools)] // API requires distinct boolean flags
pub async fn search(query: &str, detailed: bool, interactive: bool, no_aur: bool) -> Result<()> {
    search_internal(query, detailed, interactive, false, no_aur, 50).await
}

#[expect(clippy::fn_params_excessive_bools)] // API requires distinct boolean flags
pub async fn search_with_json(
    query: &str,
    detailed: bool,
    interactive: bool,
    json: bool,
    no_aur: bool,
    limit: usize,
) -> Result<()> {
    search_internal(query, detailed, interactive, json, no_aur, limit).await
}

#[expect(clippy::fn_params_excessive_bools)] // Internal function matching public API
async fn search_internal(
    query: &str,
    detailed: bool,
    _interactive: bool,
    json: bool,
    no_aur: bool,
    limit: usize,
) -> Result<()> {
    let start_time = Instant::now();

    validate_search_query(query)?;

    let official_search = async { search_official_packages(query).await };

    // Skip AUR search if --no-aur flag is set (for benchmarks/official-only searches)
    let aur_search = async {
        if no_aur {
            return Ok(Vec::new());
        }
        search_aur_packages(query, detailed).await
    };

    // Run official + AUR searches concurrently (saves ~50-100ms vs sequential)
    let (official_packages, aur_packages) = tokio::try_join!(official_search, aur_search)?;

    let mut display_packages = official_packages;
    // Deduplicate: skip AUR packages already present in official results
    let official_names: std::collections::HashSet<String> =
        display_packages.iter().map(|p| p.name.clone()).collect();
    let deduped_aur: Vec<DisplayPackage> = aur_packages
        .into_iter()
        .filter(|p: &DisplayPackage| !official_names.contains(&p.name))
        .collect();
    display_packages.extend(deduped_aur);

    // Track search with timing
    let duration_ms = start_time.elapsed().as_millis() as u64;
    crate::core::usage::track_search_timed(query, display_packages.len(), duration_ms, true);

    if json {
        let json_str = serde_json::to_string_pretty(&display_packages)
            .context("Failed to serialize search results as JSON")?;
        println!("{json_str}");
        return Ok(());
    }

    if display_packages.is_empty() {
        use crate::cli::components::Components;
        use crate::cli::packages::execute_cmd;
        execute_cmd(Components::no_results(query));
        return Ok(());
    }

    let mut stdout = std::io::BufWriter::new(std::io::stdout());
    let desc_width = description_width();
    // Modern search header - no extra blank line
    writeln!(stdout, "{}", style::header("Search Results"))?;

    for pkg in display_packages.iter().take(limit) {
        write_package_cached(&mut stdout, pkg, desc_width)?;
    }

    if display_packages.len() > limit {
        writeln!(
            stdout,
            "  {}",
            style::dim(&format!(
                "(+{} more packages...)",
                display_packages.len() - limit
            ))
        )?;
    }

    writeln!(stdout)?;
    stdout.flush()?;

    Ok(())
}

async fn search_official_packages(query: &str) -> Result<Vec<DisplayPackage>> {
    #[cfg(unix)]
    if let Ok(mut client) = DaemonClient::connect().await {
        match client.search(query, Some(50)).await {
            Ok(res) => {
                return Ok(res
                    .packages
                    .into_iter()
                    .map(|pkg| DisplayPackage {
                        name: pkg.name,
                        version: pkg.version,
                        description: pkg.description,
                        source: pkg.source,
                        votes: None,
                        popularity: None,
                        maintainer: None,
                        out_of_date: None,
                    })
                    .collect());
            }
            Err(error) => {
                tracing::debug!("Daemon search failed for {query}: {error}");
            }
        }
    }

    let pm = get_package_manager().context("Failed to initialize package manager for search")?;
    let packages = pm
        .search(query)
        .await
        .with_context(|| format!("Failed to search official repositories for {query}"))?;
    Ok(packages
        .into_iter()
        .map(DisplayPackage::from_package)
        .collect())
}

async fn search_aur_packages(query: &str, detailed: bool) -> Result<Vec<DisplayPackage>> {
    #[cfg(feature = "arch")]
    {
        if detailed {
            return crate::package_managers::search_detailed(query)
                .await
                .map(|pkgs| {
                    pkgs.into_iter()
                        .map(DisplayPackage::from_aur_detail)
                        .collect()
                })
                .with_context(|| format!("Failed to search AUR for {query}"));
        }
        let aur = AurClient::new()?;
        return aur
            .search(query)
            .await
            .map(|pkgs| pkgs.into_iter().map(DisplayPackage::from_package).collect())
            .with_context(|| format!("Failed to search AUR for {query}"));
    }
    #[cfg(not(feature = "arch"))]
    {
        let _ = (query, detailed);
        Ok(Vec::new())
    }
}

pub fn search_sync_cli(
    query: &str,
    detailed: bool,
    interactive: bool,
    no_aur: bool,
) -> Result<bool> {
    search_sync_cli_with_limit(query, detailed, interactive, no_aur, 50)
}

pub fn search_sync_cli_with_limit(
    query: &str,
    detailed: bool,
    interactive: bool,
    no_aur: bool,
    limit: usize,
) -> Result<bool> {
    if !crate::cli::packages::common::is_valid_search_query(query) {
        return Ok(false);
    }

    // Fast path: official-only search via sync client (zero runtime overhead).
    if no_aur || cfg!(not(feature = "arch")) {
        return search_sync_official_only(query, limit);
    }

    // AUR path requires async — create a minimal runtime only when necessary
    if tokio::runtime::Handle::try_current().is_ok() {
        return Ok(false);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(search_internal(
        query,
        detailed,
        interactive,
        false,
        no_aur,
        limit,
    ))?;
    Ok(true)
}

/// Sync-only search: daemon IPC via `PooledSyncClient`, no tokio runtime.
fn search_sync_official_only(query: &str, limit: usize) -> Result<bool> {
    #[cfg(not(unix))]
    {
        return Ok(false); // Daemon not supported on Windows
    }

    #[cfg(unix)]
    {
        let Ok(mut client) = PooledSyncClient::acquire() else {
            return Ok(false); // Daemon not running; caller falls back to async
        };

        let Ok(res) = client.search(query, Some(limit)) else {
            return Ok(false);
        };

        if res.packages.is_empty() {
            use crate::cli::components::Components;
            use crate::cli::packages::execute_cmd;
            execute_cmd(Components::no_results(query));
            return Ok(true);
        }

        let mut stdout = std::io::BufWriter::new(std::io::stdout());
        let desc_width = description_width();
        writeln!(stdout, "{}", style::header("Search Results"))?;

        for pkg in res.packages.iter().take(limit) {
            write_daemon_package(&mut stdout, pkg, desc_width)?;
        }

        if res.total > limit {
            writeln!(
                stdout,
                "  {}",
                style::dim(&format!("(+{} more packages...)", res.total - limit))
            )?;
        }

        writeln!(stdout)?;
        stdout.flush()?;
        Ok(true)
    }
}

#[inline]
fn write_package_cached<W: Write>(
    w: &mut W,
    pkg: &DisplayPackage,
    desc_width: usize,
) -> std::io::Result<()> {
    let source_style = match pkg.source.as_str() {
        "AUR" => style::warning(&pkg.source),
        _ => style::info(&pkg.source),
    };

    write!(
        w,
        "  {} {} ({}) - {}",
        style::package(&pkg.name),
        style::version(&pkg.version),
        source_style,
        style::dim(&crate::cli::packages::common::truncate(
            &pkg.description,
            desc_width
        ))
    )?;

    if let Some(votes) = pkg.votes {
        write!(
            w,
            " {} {}",
            style::info(&format!("↑{votes}")),
            style::info(&format!("{:.1}%", pkg.popularity.unwrap_or(0.0)))
        )?;
    }
    if pkg.out_of_date == Some(true) {
        write!(w, " {}", style::error("[OUT OF DATE]"))?;
    }

    writeln!(w)
}

#[cfg(unix)]
#[inline]
fn write_daemon_package<W: Write>(
    w: &mut W,
    pkg: &crate::daemon::protocol::PackageInfo,
    desc_width: usize,
) -> std::io::Result<()> {
    let source_style = match pkg.source.as_str() {
        "AUR" => style::warning(&pkg.source),
        _ => style::info(&pkg.source),
    };

    writeln!(
        w,
        "  {} {} ({}) - {}",
        style::package(&pkg.name),
        style::version(&pkg.version),
        source_style,
        style::dim(&crate::cli::packages::common::truncate(
            &pkg.description,
            desc_width
        ))
    )
}

#[cfg(test)]
mod tests {
    fn format_package(pkg: &super::DisplayPackage) -> String {
        let source_style = match pkg.source.as_str() {
            "AUR" => super::style::warning(&pkg.source),
            _ => super::style::info(&pkg.source),
        };

        format!(
            "  {} {} ({}) - {}",
            super::style::package(&pkg.name),
            super::style::version(&pkg.version),
            source_style,
            super::style::dim(&crate::cli::packages::common::truncate(
                &pkg.description,
                50
            ))
        )
    }
    use super::*;

    #[test]
    #[cfg(feature = "arch")]
    fn test_display_package_from_package() {
        let pkg = Package {
            name: "firefox".to_string(),
            version: alpm_types::Version::from(
                "123.0-1".parse::<alpm_types::FullVersion>().unwrap(),
            ),
            description: "Fast web browser".to_string(),
            source: crate::core::PackageSource::Official,
            installed: false,
        };

        let display = DisplayPackage::from_package(pkg);
        assert_eq!(display.name, "firefox");
        assert_eq!(display.version, "123.0-1");
        assert_eq!(display.description, "Fast web browser");
        assert_eq!(display.source, "Official");
    }

    #[test]
    fn test_format_package_aur() {
        let pkg = DisplayPackage {
            name: "yay".to_string(),
            version: "12.0.0".to_string(),
            description: "AUR helper".to_string(),
            source: "AUR".to_string(),
            votes: None,
            popularity: None,
            maintainer: None,
            out_of_date: None,
        };

        let formatted = format_package(&pkg);
        assert!(formatted.contains("yay"));
        assert!(formatted.contains("12.0.0"));
        assert!(formatted.contains("AUR"));
    }

    #[test]
    fn test_format_package_official() {
        let pkg = DisplayPackage {
            name: "pacman".to_string(),
            version: "6.0.0".to_string(),
            description: "Package manager".to_string(),
            source: "core".to_string(),
            votes: None,
            popularity: None,
            maintainer: None,
            out_of_date: None,
        };

        let formatted = format_package(&pkg);
        assert!(formatted.contains("pacman"));
        assert!(formatted.contains("6.0.0"));
        assert!(formatted.contains("core"));
    }

    #[tokio::test]
    async fn test_search_query_too_long() {
        let long_query = "a".repeat(101);
        let result = search_internal(&long_query, false, false, false, false, 50).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Search query too long")
        );
    }

    #[tokio::test]
    async fn test_search_query_control_chars() {
        let result = search_internal("test\x00query", false, false, false, false, 50).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("invalid characters")
        );
    }

    #[tokio::test]
    async fn test_search_query_path_traversal() {
        let result = search_internal("../etc/passwd", false, false, false, false, 50).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path traversal"));
    }

    #[tokio::test]
    async fn test_search_query_shell_metacharacters() {
        let result = search_internal("test;rm -rf", false, false, false, false, 50).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("shell metacharacters")
        );
    }

    #[test]
    fn test_search_sync_cli_validation() {
        assert!(!search_sync_cli("a".repeat(101).as_str(), false, false, true).unwrap());

        assert!(!search_sync_cli("test\x00query", false, false, true).unwrap());

        assert!(!search_sync_cli("../passwd", false, false, true).unwrap());

        assert!(!search_sync_cli("test;ls", false, false, true).unwrap());
    }
}
