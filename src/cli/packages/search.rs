use std::io::Write;

use anyhow::Result;

use crate::cli::style;
use crate::core::Package;
use crate::core::client::DaemonClient;
use crate::core::env::distro::use_debian_backend;
use crate::package_managers::get_package_manager;

#[cfg(feature = "arch")]
use crate::package_managers::AurClient;

struct DisplayPackage {
    name: String,
    version: String,
    description: String,
    source: String,
}

impl DisplayPackage {
    fn from_package(p: Package) -> Self {
        Self {
            name: p.name,
            version: p.version.to_string(),
            description: p.description,
            source: p.source.to_string(),
        }
    }

    #[cfg(feature = "arch")]
    fn from_aur_detail(p: crate::package_managers::AurPackageDetail) -> Self {
        Self {
            name: p.name,
            version: p.version,
            description: p.description.unwrap_or_default(),
            source: "AUR".to_string(),
        }
    }
}

pub async fn search(query: &str, detailed: bool, _interactive: bool) -> Result<()> {
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

    if let Ok(mut client) = DaemonClient::connect().await {
        if let Ok(res) = client.search(query, Some(50)).await {
            display_daemon_results(res, query);
            return Ok(());
        }
    }

    let pm = get_package_manager();
    let packages = pm.search(query).await?;

    let mut display_packages: Vec<DisplayPackage> = packages
        .into_iter()
        .map(DisplayPackage::from_package)
        .collect();

    #[cfg(feature = "arch")]
    if !use_debian_backend() {
        let aur_packages = if detailed {
            if let Ok(details) = crate::package_managers::search_detailed(query).await {
                details
                    .into_iter()
                    .map(DisplayPackage::from_aur_detail)
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            let aur = AurClient::new();
            if let Ok(pkgs) = aur.search(query).await {
                pkgs.into_iter().map(DisplayPackage::from_package).collect()
            } else {
                Vec::new()
            }
        };
        display_packages.extend(aur_packages);
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

/// Synchronous fast-path for search
pub fn search_sync_cli(query: &str, detailed: bool, interactive: bool) -> Result<bool> {
    // Basic validation
    if query.len() > 100 || query.chars().any(char::is_control) {
        return Ok(false);
    }

    // Try to run search using block_on
    // For a true fast-path we might want a dedicated sync search that only hits the local index
    // but for now this restores functionality.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(search(query, detailed, interactive))?;
    Ok(true)
}

fn display_daemon_results(res: crate::daemon::protocol::SearchResult, query: &str) {
    if res.packages.is_empty() {
        use crate::cli::components::Components;
        use crate::cli::packages::execute_cmd;
        execute_cmd(Components::no_results(query));
        return;
    }

    println!("\n{}", style::header("OMG (Cached)"));

    for pkg in res.packages.iter().take(20) {
        let source = if pkg.source == "official" {
            style::info(&pkg.source)
        } else {
            style::warning(&pkg.source)
        };

        println!(
            "  {} {} ({}) - {}",
            style::package(&pkg.name),
            style::version(&pkg.version),
            source,
            style::dim(&crate::cli::packages::common::truncate(
                &pkg.description,
                50
            ))
        );
    }
    if res.packages.len() > 20 {
        println!(
            "  {}",
            style::dim(&format!("(+{} more packages...)", res.packages.len() - 20))
        );
    }
    println!();
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
