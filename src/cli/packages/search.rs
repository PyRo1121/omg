use std::io::Write;

use anyhow::Result;
use serde::Serialize;

use crate::cli::style;
use crate::core::Package;
#[cfg(unix)]
use crate::core::client::{DaemonClient, PooledSyncClient};
use crate::package_managers::get_package_manager;

#[cfg(feature = "arch")]
use crate::package_managers::AurClient;

#[derive(Serialize)]
struct DisplayPackage {
    name: String,
    version: String,
    description: String,
    source: String,
}

impl DisplayPackage {
    #[allow(clippy::implicit_clone)] // Version type varies by feature flag
    fn from_package(p: Package) -> Self {
        Self {
            name: p.name,
            version: p.version.to_string(),
            description: p.description,
            source: p.source.to_string(),
        }
    }
}

#[allow(clippy::fn_params_excessive_bools)] // API requires distinct boolean flags
pub async fn search(query: &str, detailed: bool, interactive: bool, no_aur: bool) -> Result<()> {
    search_internal(query, detailed, interactive, false, no_aur).await
}

#[allow(clippy::fn_params_excessive_bools)] // API requires distinct boolean flags
pub async fn search_with_json(
    query: &str,
    detailed: bool,
    interactive: bool,
    json: bool,
    no_aur: bool,
) -> Result<()> {
    search_internal(query, detailed, interactive, json, no_aur).await
}

#[allow(clippy::fn_params_excessive_bools)] // Internal function matching public API
async fn search_internal(
    query: &str,
    _detailed: bool,
    _interactive: bool,
    json: bool,
    no_aur: bool,
) -> Result<()> {
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

    let official_search = async {
        let mut results = Vec::with_capacity(50); // Pre-allocate for typical search results
        #[cfg(unix)]
        if let Ok(mut client) = DaemonClient::connect().await
            && let Ok(res) = client.search(query, Some(50)).await
        {
            for pkg in res.packages {
                results.push(DisplayPackage {
                    name: pkg.name,
                    version: pkg.version,
                    description: pkg.description,
                    source: pkg.source,
                });
            }
        }
        #[cfg(not(unix))]
        if let Ok(packages) = get_package_manager().search(query).await {
            results.extend(packages.into_iter().map(DisplayPackage::from_package));
        }
        #[cfg(unix)]
        if results.is_empty()
            && let Ok(packages) = get_package_manager().search(query).await
        {
            results.extend(packages.into_iter().map(DisplayPackage::from_package));
        }
        results
    };

    // Skip AUR search if --no-aur flag is set (for benchmarks/official-only searches)
    let aur_packages = if no_aur {
        Vec::new()
    } else {
        #[cfg(feature = "arch")]
        {
            let aur = AurClient::new();
            aur.search(query)
                .await
                .map(|pkgs| pkgs.into_iter().map(DisplayPackage::from_package).collect())
                .unwrap_or_default()
        }
        #[cfg(not(feature = "arch"))]
        {
            Vec::new()
        }
    };

    let official_packages = official_search.await;

    let mut display_packages = official_packages;
    display_packages.extend(aur_packages);

    if json {
        let json_str =
            serde_json::to_string_pretty(&display_packages).unwrap_or_else(|_| "[]".to_string());
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
    writeln!(stdout, "\n{}", style::header("Search Results"))?;

    for pkg in display_packages.iter().take(50) {
        writeln!(stdout, "{}", format_package(pkg))?;
    }

    if display_packages.len() > 50 {
        writeln!(
            stdout,
            "  {}",
            style::dim(&format!(
                "(+{} more packages...)",
                display_packages.len() - 50
            ))
        )?;
    }

    writeln!(stdout)?;
    stdout.flush()?;

    Ok(())
}

pub fn search_sync_cli(
    query: &str,
    _detailed: bool,
    _interactive: bool,
    no_aur: bool,
) -> Result<bool> {
    if query.len() > 100 || query.chars().any(char::is_control) {
        return Ok(false);
    }
    if query.contains('/') || query.contains('\\') || query.contains("..") {
        return Ok(false);
    }
    if query.chars().any(|c| ";|&><$".contains(c)) {
        return Ok(false);
    }

    // Fast path: official-only search via sync client (zero runtime overhead).
    // When AUR is needed, fall back to the async path which requires a runtime.
    if no_aur {
        return search_sync_official_only(query);
    }

    // AUR path requires async — create a minimal runtime only when necessary
    if tokio::runtime::Handle::try_current().is_ok() {
        return Ok(false);
    }

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(search(query, false, false, no_aur))?;
    Ok(true)
}

/// Sync-only search: daemon IPC via `PooledSyncClient`, no tokio runtime.
fn search_sync_official_only(query: &str) -> Result<bool> {
    #[cfg(not(unix))]
    {
        return Ok(false); // Daemon not supported on Windows
    }

    #[cfg(unix)]
    {
        let Ok(mut client) = PooledSyncClient::acquire() else {
            return Ok(false); // Daemon not running; caller falls back to async
        };

        let Ok(res) = client.search(query, Some(50)) else {
            return Ok(false);
        };

        if res.packages.is_empty() {
            use crate::cli::components::Components;
            use crate::cli::packages::execute_cmd;
            execute_cmd(Components::no_results(query));
            return Ok(true);
        }

        let mut stdout = std::io::BufWriter::new(std::io::stdout());
        writeln!(stdout, "\n{}", style::header("Search Results"))?;

        for pkg in res.packages.iter().take(50) {
            let source_style = if pkg.source == "AUR" {
                style::warning(&pkg.source)
            } else {
                style::info(&pkg.source)
            };
            writeln!(
                stdout,
                "  {} {} ({}) - {}",
                style::package(&pkg.name),
                style::version(&pkg.version),
                source_style,
                style::dim(&crate::cli::packages::common::truncate(
                    &pkg.description,
                    50
                ))
            )?;
        }

        if res.total > 50 {
            writeln!(
                stdout,
                "  {}",
                style::dim(&format!("(+{} more packages...)", res.total - 50))
            )?;
        }

        writeln!(stdout)?;
        stdout.flush()?;
        Ok(true)
    }
}

fn format_package(pkg: &DisplayPackage) -> String {
    let source_style = match pkg.source.as_str() {
        "AUR" => style::warning(&pkg.source),
        _ => style::info(&pkg.source),
    };

    format!(
        "  {} {} ({}) - {}",
        style::package(&pkg.name),
        style::version(&pkg.version),
        source_style,
        style::dim(&crate::cli::packages::common::truncate(
            &pkg.description,
            50
        ))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_package_from_package() {
        let pkg = Package {
            name: "firefox".to_string(),
            version: alpm_types::Version::from("123.0-1".parse::<alpm_types::FullVersion>().unwrap()),
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
        };

        let formatted = format_package(&pkg);
        assert!(formatted.contains("pacman"));
        assert!(formatted.contains("6.0.0"));
        assert!(formatted.contains("core"));
    }

    #[tokio::test]
    async fn test_search_query_too_long() {
        let long_query = "a".repeat(101);
        let result = search_internal(&long_query, false, false, false, false).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Search query too long"));
    }

    #[tokio::test]
    async fn test_search_query_control_chars() {
        let result = search_internal("test\x00query", false, false, false, false).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid characters"));
    }

    #[tokio::test]
    async fn test_search_query_path_traversal() {
        let result = search_internal("../etc/passwd", false, false, false, false).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("path traversal"));
    }

    #[tokio::test]
    async fn test_search_query_shell_metacharacters() {
        let result = search_internal("test;rm -rf", false, false, false, false).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("shell metacharacters"));
    }

    #[test]
    fn test_search_sync_cli_validation() {
        assert!(!search_sync_cli("a".repeat(101).as_str(), false, false, true).unwrap());

        assert!(!search_sync_cli("test\x00query", false, false, true).unwrap());

        assert!(!search_sync_cli("../passwd", false, false, true).unwrap());

        assert!(!search_sync_cli("test;ls", false, false, true).unwrap());
    }
}
