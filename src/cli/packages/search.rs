use std::io::Write;

use anyhow::Result;
use serde::Serialize;

use crate::cli::style;
use crate::core::Package;
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
        let mut results = Vec::new();
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
        } else if let Ok(packages) = get_package_manager().search(query).await {
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
            if let Ok(pkgs) = aur.search(query).await {
                pkgs.into_iter()
                    .map(DisplayPackage::from_package)
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        }
        #[cfg(not(feature = "arch"))]
        Vec::new()
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

fn format_package(pkg: &DisplayPackage) -> String {
    let source_style = if pkg.source == "AUR" {
        style::warning(&pkg.source)
    } else {
        style::info(&pkg.source)
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
