use std::io::Write;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::cli::packages::common::{description_width, validate_search_query};
use crate::cli::style;
use crate::core::format::truncate;
use crate::core::{Package, PackageSource};
use crate::package_managers::{VersionDisplay, get_package_manager};

#[cfg(unix)]
use crate::core::client::{DaemonClient, SyncDaemonClient};

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
    fn from_package(p: Package) -> Self {
        Self {
            name: p.name,
            version: p.version.version_string(),
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
            maintainer: p.maintainer,
            out_of_date: Some(p.out_of_date.is_some()),
        }
    }
}

pub async fn search_with_json(
    query: &str,
    detailed: bool,
    json: bool,
    no_aur: bool,
    limit: usize,
) -> Result<()> {
    search_internal(query, detailed, json, no_aur, limit).await
}

async fn search_internal(
    query: &str,
    detailed: bool,
    json: bool,
    no_aur: bool,
    limit: usize,
) -> Result<()> {
    validate_search_query(query)?;

    let official_search = async { search_official_packages(query, limit).await };

    // Skip AUR search if --no-aur flag is set (for benchmarks/official-only searches)
    let aur_search = async {
        if no_aur {
            return Ok(Vec::new());
        }
        tokio::time::timeout(
            std::time::Duration::from_secs(4),
            search_aur_packages(query, detailed),
        )
        .await
        .context("AUR search timed out")?
    };

    // Official results are authoritative and remain useful when optional AUR
    // enrichment is unavailable. Run both concurrently, but bound AUR latency.
    let (official_result, aur_result) = tokio::join!(official_search, aur_search);
    let (mut display_packages, official_total) = official_result?;
    let aur_packages = match aur_result {
        Ok(packages) => packages,
        Err(error) if !display_packages.is_empty() => {
            tracing::debug!("AUR search unavailable; returning official results: {error}");
            Vec::new()
        }
        Err(error) => return Err(error).context("Failed to search AUR packages"),
    };
    // Deduplicate: skip AUR packages already present in official results.
    // Borrow the names instead of cloning them; this runs on every search.
    let official_names: std::collections::HashSet<&str> =
        display_packages.iter().map(|p| p.name.as_str()).collect();
    let deduped_aur: Vec<DisplayPackage> = aur_packages
        .into_iter()
        .filter(|p| !official_names.contains(p.name.as_str()))
        .collect();
    let aur_count = deduped_aur.len();
    display_packages.extend(deduped_aur);
    let total_matches = official_total.saturating_add(aur_count);

    crate::core::usage::track_search_result(true);
    truncate_search_results(&mut display_packages, limit);

    if json {
        let json_str = serde_json::to_string_pretty(&display_packages)
            .context("Failed to serialize search results as JSON")?;
        println!("{json_str}");
        return Ok(());
    }

    if display_packages.is_empty() {
        use crate::cli::components::Components;
        use crate::cli::packages::execute_cmd;
        execute_cmd(Components::no_results(query))?;
        return Ok(());
    }

    let mut stdout = std::io::BufWriter::new(std::io::stdout());
    let desc_width = description_width();
    // Modern search header - no extra blank line
    writeln!(stdout, "{}", style::header("Search Results"))?;

    for pkg in display_packages.iter().take(limit) {
        write_package_line(&mut stdout, pkg, desc_width)?;
    }

    if total_matches > limit {
        writeln!(
            stdout,
            "  {}",
            style::dim(&format!("(+{} more packages...)", total_matches - limit))
        )?;
    }

    writeln!(stdout)?;
    stdout.flush()?;

    Ok(())
}

fn truncate_search_results(packages: &mut Vec<DisplayPackage>, limit: usize) {
    packages.truncate(limit);
}

async fn search_official_packages(
    query: &str,
    limit: usize,
) -> Result<(Vec<DisplayPackage>, usize)> {
    #[cfg(unix)]
    if let Ok(mut client) = DaemonClient::connect().await {
        match client.search(query, Some(limit)).await {
            Ok(res) => {
                return Ok((
                    res.packages
                        .into_iter()
                        .map(|pkg| DisplayPackage {
                            name: pkg.name,
                            version: pkg.version,
                            description: pkg.description,
                            source: pkg.source.label().to_string(),
                            votes: None,
                            popularity: None,
                            maintainer: None,
                            out_of_date: None,
                        })
                        .collect(),
                    res.total,
                ));
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
    let total = packages.len();
    Ok((
        packages
            .into_iter()
            .map(DisplayPackage::from_package)
            .collect(),
        total,
    ))
}

#[cfg(feature = "arch")]
async fn search_aur_packages(query: &str, detailed: bool) -> Result<Vec<DisplayPackage>> {
    if crate::core::paths::test_mode() {
        return Ok(Vec::new());
    }

    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return Ok(Vec::new());
    }

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
    aur.search(query)
        .await
        .map(|pkgs| pkgs.into_iter().map(DisplayPackage::from_package).collect())
        .with_context(|| format!("Failed to search AUR for {query}"))
}

#[cfg(not(feature = "arch"))]
fn search_aur_packages(
    _query: &str,
    _detailed: bool,
) -> std::future::Ready<Result<Vec<DisplayPackage>>> {
    std::future::ready(Ok(Vec::new()))
}

pub fn search_sync_cli_with_limit(
    query: &str,
    detailed: bool,
    no_aur: bool,
    limit: usize,
) -> Result<bool> {
    if !crate::cli::packages::common::is_valid_search_query(query) {
        return Ok(false);
    }

    // Fast path: official-only search via sync client (zero runtime overhead).
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return search_sync_official_only(query, limit);
    }

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
    rt.block_on(async {
        search_internal(query, detailed, false, no_aur, limit).await?;
        // This runtime is about to be dropped, so a fire-and-forget usage
        // task would be cancelled. Flush at this owned shutdown boundary.
        crate::core::usage::sync_usage_now().await;
        Ok::<(), anyhow::Error>(())
    })?;
    Ok(true)
}

/// Sync-only search: daemon IPC via `SyncDaemonClient`, no tokio runtime.
fn search_sync_official_only(query: &str, limit: usize) -> Result<bool> {
    #[cfg(not(unix))]
    {
        return Ok(false); // Daemon not supported on Windows
    }

    #[cfg(unix)]
    {
        let Ok(mut client) = SyncDaemonClient::acquire() else {
            return Ok(false); // Daemon not running; caller falls back to async
        };

        let Ok(res) = client.search(query, Some(limit)) else {
            return Ok(false);
        };

        if res.packages.is_empty() {
            use crate::cli::components::Components;
            use crate::cli::packages::execute_cmd;
            execute_cmd(Components::no_results(query))?;
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

fn styled_source(source: &str) -> String {
    if PackageSource::from_label(source) == Some(PackageSource::Aur) {
        style::warning(source)
    } else {
        style::info(source)
    }
}

#[inline]
fn write_package_line<W: Write>(
    w: &mut W,
    pkg: &DisplayPackage,
    desc_width: usize,
) -> std::io::Result<()> {
    let source_style = styled_source(&pkg.source);

    write!(
        w,
        "  {} {} ({}) - {}",
        style::package(&pkg.name),
        style::version(&pkg.version),
        source_style,
        style::dim(&truncate(
            &style::sanitize_terminal_text(&pkg.description),
            desc_width,
        ))
    )?;

    if let Some(votes) = pkg.votes {
        write!(
            w,
            " {} {}",
            style::info(&format!("↑{votes}")),
            style::info(&format!("{:.1}", pkg.popularity.unwrap_or(0.0)))
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
    let source_style = styled_source(pkg.source.label());

    writeln!(
        w,
        "  {} {} ({}) - {}",
        style::package(&pkg.name),
        style::version(&pkg.version),
        source_style,
        style::dim(&truncate(
            &style::sanitize_terminal_text(&pkg.description),
            desc_width,
        ))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Format a package through the real production writer so the tests
    /// assert the exact output users see instead of a duplicated format
    /// string.
    fn format_package(pkg: &super::DisplayPackage) -> String {
        let mut buf = Vec::new();
        write_package_line(&mut buf, pkg, 50).expect("in-memory writer cannot fail");
        String::from_utf8(buf).expect("writer only emits UTF-8")
    }

    #[test]
    fn output_limit_applies_before_json_serialization() {
        let mut packages = (0..3)
            .map(|index| DisplayPackage {
                name: format!("package-{index}"),
                version: "1".to_string(),
                description: String::new(),
                source: "official".to_string(),
                votes: None,
                popularity: None,
                maintainer: None,
                out_of_date: None,
            })
            .collect::<Vec<_>>();

        truncate_search_results(&mut packages, 2);
        let json = serde_json::to_value(&packages).expect("serialize search results");
        assert_eq!(json.as_array().map(Vec::len), Some(2));
    }

    #[test]
    fn package_writer_strips_terminal_control_sequences() {
        let package = DisplayPackage {
            name: "safe".to_string(),
            version: "1".to_string(),
            description: "normal\x1b]52;c;secret\x07\nforged".to_string(),
            source: "official".to_string(),
            votes: None,
            popularity: None,
            maintainer: None,
            out_of_date: None,
        };

        let output = format_package(&package);
        assert!(!output.contains('\x1b'));
        assert!(!output.contains('\x07'));
        assert!(!output.contains("\nforged"));
    }

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
    fn aur_popularity_is_not_rendered_as_a_percentage() {
        let package = DisplayPackage {
            name: "yay".to_string(),
            version: "12.0.0".to_string(),
            description: "AUR helper".to_string(),
            source: "AUR".to_string(),
            votes: Some(10),
            popularity: Some(0.73),
            maintainer: None,
            out_of_date: None,
        };
        let formatted = format_package(&package);
        assert!(formatted.contains("0.7"));
        assert!(!formatted.contains("0.7%"));
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
        let result = search_internal(&long_query, false, false, false, 50).await;
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
        let result = search_internal("test\x00query", false, false, false, 50).await;
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
        let result = search_internal("../etc/passwd", false, false, false, 50).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path traversal"));
    }

    #[tokio::test]
    async fn test_search_query_shell_metacharacters() {
        let result = search_internal("test;rm -rf", false, false, false, 50).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("shell metacharacters")
        );
    }

    #[test]
    fn test_search_sync_cli_with_limit_validation() {
        assert!(!search_sync_cli_with_limit("a".repeat(101).as_str(), false, true, 50).unwrap());

        assert!(!search_sync_cli_with_limit("test\x00query", false, true, 50).unwrap());

        assert!(!search_sync_cli_with_limit("../passwd", false, true, 50).unwrap());

        assert!(!search_sync_cli_with_limit("test;ls", false, true, 50).unwrap());
    }
}
