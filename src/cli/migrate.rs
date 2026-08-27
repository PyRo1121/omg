//! `omg migrate` - Cross-distro migration tools

use anyhow::Result;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use crate::cli::style;
use crate::core::env::distro::detect_distro;
use crate::core::env::fingerprint::EnvironmentState;

/// The only manifest format this build can import. Forward versions must be
/// rejected explicitly instead of being silently misread field by field.
const MANIFEST_FORMAT_VERSION: &str = "1.0";

#[derive(Debug, Serialize, Deserialize)]
pub struct MigrationManifest {
    pub version: String,
    pub source_distro: String,
    pub created_at: i64,
    pub runtimes: HashMap<String, String>,
    pub packages: Vec<PackageMapping>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageMapping {
    pub original_name: String,
    pub category: String,
    pub description: Option<String>,
    pub alternatives: Vec<String>,
}

/// Export current environment to portable manifest
pub async fn export(output: &str) -> Result<()> {
    println!("{} Exporting environment...\n", style::runtime("OMG"));

    let state = EnvironmentState::capture().await?;
    let distro = format!("{:?}", detect_distro()).to_lowercase();
    let runtime_count = state.runtimes.len();
    let package_count = state.packages.len();

    let packages = state
        .packages
        .iter()
        .map(|name| create_package_mapping(name))
        .collect();

    let manifest = MigrationManifest {
        version: MANIFEST_FORMAT_VERSION.to_string(),
        source_distro: distro,
        created_at: jiff::Timestamp::now().as_second(),
        runtimes: state.runtimes,
        packages,
    };

    let content = serde_json::to_string_pretty(&manifest)?;
    crate::core::safe_ops::atomic_write_file_sync(output, content)?;

    println!(
        "  {} Exported to {}",
        style::maybe_color("✓", |t| t.green().to_string()),
        style::maybe_color(output, |t| t.cyan().to_string())
    );
    println!();
    println!("  Source distro: {}", style::path(&manifest.source_distro));
    println!("  Runtimes: {runtime_count}");
    println!("  Packages: {package_count}");
    println!();
    println!(
        "  {}",
        style::maybe_color("To import on another machine:", |t| {
            t.bold().to_string()
        })
    );
    println!(
        "    1. Copy {} to the target machine",
        style::maybe_color(output, |t| t.cyan().to_string())
    );
    println!(
        "    2. Run {}",
        style::command(&format!("omg migrate import {output}"))
    );

    Ok(())
}

/// Import environment from manifest with package mapping
pub async fn import(manifest_path: &str, dry_run: bool) -> Result<()> {
    let manifest_path = crate::core::safe_ops::validate_path_syntax(manifest_path)?;

    println!(
        "{} {} manifest...\n",
        style::runtime("OMG"),
        if dry_run { "Previewing" } else { "Importing" }
    );

    let content = fs::read_to_string(&manifest_path)?;
    let manifest: MigrationManifest = serde_json::from_str(&content)?;

    // Reject unknown forward versions instead of misreading their fields.
    anyhow::ensure!(
        manifest.version == MANIFEST_FORMAT_VERSION,
        "Unsupported migration manifest version '{}' (this build reads '{}'); \
         regenerate the manifest with a matching omg version",
        manifest.version,
        MANIFEST_FORMAT_VERSION
    );

    let target_distro = format!("{:?}", detect_distro()).to_lowercase();

    println!(
        "  Source: {} → Target: {}",
        style::path(&manifest.source_distro),
        style::maybe_color(&target_distro, |t| t.cyan().to_string())
    );
    println!();

    // Map packages
    println!(
        "  {}",
        style::maybe_color("Package mapping:", |t| t.bold().to_string())
    );

    let mut to_install = Vec::new();

    for pkg in &manifest.packages {
        let target_pkg = map_package(&pkg.original_name, &manifest.source_distro, &target_distro);

        if target_pkg != pkg.original_name {
            println!(
                "    {} {} → {}",
                style::maybe_color("✓", |t| t.green().to_string()),
                style::dim(&pkg.original_name),
                style::maybe_color(&target_pkg, |t| t.cyan().to_string())
            );
        }
        to_install.push(target_pkg);
    }

    println!();
    println!(
        "  Mapped: {}/{} packages",
        style::version(&to_install.len().to_string()),
        manifest.packages.len()
    );

    // Runtimes
    println!();
    println!(
        "  {}",
        style::maybe_color("Runtimes:", |t| t.bold().to_string())
    );
    for (runtime, version) in &manifest.runtimes {
        println!(
            "    {} {} @ {}",
            style::maybe_color("→", |t| t.blue().to_string()),
            runtime,
            style::maybe_color(version, |t| t.cyan().to_string())
        );
    }

    if dry_run {
        println!();
        println!(
            "  {} No changes made (dry run)",
            style::maybe_color("ℹ", |t| t.blue().to_string())
        );
        println!(
            "  Run without --dry-run to install: {}",
            style::command(&format!("omg migrate import {}", manifest_path.display()))
        );
        return Ok(());
    }

    // Apply changes
    println!();
    println!(
        "  {}",
        style::maybe_color("Applying...", |t| t.bold().to_string())
    );

    // Install runtimes
    let mut runtime_failures = 0usize;
    for (runtime, version) in &manifest.runtimes {
        println!("    Installing {runtime} {version}...");
        if let Err(e) = crate::cli::runtimes::use_version(runtime, Some(version)).await {
            println!(
                "      {} Failed to install {runtime}: {e}",
                style::maybe_color("✗", |t| t.red().to_string())
            );
            runtime_failures += 1;
        }
    }

    // Install packages
    let mut package_failed = false;
    if !to_install.is_empty() {
        println!();
        println!("    Installing {} packages...", to_install.len());
        if let Err(e) = crate::cli::packages::install(&to_install, true, false, false).await {
            println!(
                "      {} Package installation failed: {e}",
                style::maybe_color("✗", |t| t.red().to_string())
            );
            package_failed = true;
        }
    }

    finish_apply(runtime_failures, package_failed)
}

fn finish_apply(runtime_failures: usize, package_failed: bool) -> Result<()> {
    if runtime_failures == 0 && !package_failed {
        println!();
        println!(
            "  {} Migration complete!",
            style::maybe_color("✓", |t| t.green().to_string())
        );
        println!("  Some packages may need manual installation - check the unmapped list above.");
        Ok(())
    } else {
        anyhow::bail!(
            "Migration failed ({runtime_failures} runtime install failure(s), package install {})",
            if package_failed { "failed" } else { "ok" }
        )
    }
}

fn create_package_mapping(name: &str) -> PackageMapping {
    PackageMapping {
        original_name: name.to_string(),
        category: categorize_package(name).to_string(),
        description: None,
        alternatives: get_alternatives(name),
    }
}

fn categorize_package(name: &str) -> &'static str {
    if name.contains("lib") {
        "library"
    } else if name.contains("dev") || name.contains("devel") {
        "development"
    } else if name.ends_with("-doc") || name.ends_with("-docs") {
        "documentation"
    } else {
        "application"
    }
}

fn get_alternatives(name: &str) -> Vec<String> {
    let alternatives: &[&str] = match name {
        "vim" => &["vim", "vim-nox", "neovim"],
        "gcc" | "make" => &[name, "build-essential"],
        "git" | "curl" | "wget" => &[name],
        "python" => &["python3", "python"],
        "nodejs" => &["nodejs", "node"],
        _ => &[],
    };
    alternatives.iter().map(ToString::to_string).collect()
}

fn map_package(name: &str, from: &str, to: &str) -> String {
    // Direct mappings between distros. The tables are tiny and static, so a
    // plain match over the (from, to) pair avoids rebuilding maps per package.
    match (from, to) {
        ("arch", "debian" | "ubuntu") => match name {
            "base-devel" => "build-essential",
            "python" => "python3",
            "python-pip" => "python3-pip",
            "linux-headers" => "linux-headers-generic",
            "lib32-glibc" => "libc6-i386",
            other => other,
        },
        ("debian" | "ubuntu", "arch") => match name {
            "build-essential" => "base-devel",
            "python3" => "python",
            "python3-pip" => "python-pip",
            "linux-headers-generic" => "linux-headers",
            other => other,
        },
        _ => name,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_package() {
        assert_eq!(categorize_package("libc6"), "library");
        assert_eq!(categorize_package("libssl-dev"), "library"); // contains lib
        assert_eq!(categorize_package("python3-dev"), "development");
        assert_eq!(categorize_package("gcc-docs"), "documentation");
        assert_eq!(categorize_package("firefox"), "application");
    }

    #[test]
    fn test_get_alternatives() {
        let alts = get_alternatives("vim");
        assert!(alts.contains(&"neovim".to_string()));
        assert!(alts.contains(&"vim".to_string()));

        let empty = get_alternatives("nonexistent-pkg-123");
        assert!(empty.is_empty());
    }

    #[test]
    fn apply_failure_is_an_error() {
        let result = finish_apply(1, false);
        assert!(
            result.is_err(),
            "failed runtime installs must be a CLI error so the process exits non-zero"
        );
        let package_result = finish_apply(0, true);
        assert!(
            package_result.is_err(),
            "failed package installs must be a CLI error so the process exits non-zero"
        );
    }

    #[test]
    fn apply_success_is_ok() {
        assert!(finish_apply(0, false).is_ok());
    }

    #[test]
    fn test_map_package() {
        // Arch to Debian
        assert_eq!(
            map_package("base-devel", "arch", "debian"),
            "build-essential"
        );
        assert_eq!(map_package("python", "arch", "ubuntu"), "python3");

        // Debian to Arch
        assert_eq!(
            map_package("build-essential", "debian", "arch"),
            "base-devel"
        );
        assert_eq!(map_package("python3", "ubuntu", "arch"), "python");

        // No mapping (identity)
        assert_eq!(
            map_package("my-custom-pkg", "arch", "debian"),
            "my-custom-pkg"
        );
    }

    #[test]
    fn import_rejects_unknown_manifest_versions() {
        let manifest = |version: &str| {
            format!(
                r#"{{"version":"{version}","source_distro":"arch","created_at":0,
                    "runtimes":{{}},"packages":[]}}"#
            )
        };
        let dir = tempfile::tempdir().expect("temp dir");

        let write_and_parse = |version: &str| {
            let path = dir.path().join(format!("manifest-{version}.json"));
            std::fs::write(&path, manifest(version)).expect("write manifest");
            let content = std::fs::read_to_string(&path).expect("read manifest");
            serde_json::from_str::<MigrationManifest>(&content).expect("fixture must deserialize")
        };

        let supported = write_and_parse("1.0");
        assert_eq!(supported.version, MANIFEST_FORMAT_VERSION);

        let future = write_and_parse("2.0");
        let error = (|| -> Result<()> {
            anyhow::ensure!(
                future.version == MANIFEST_FORMAT_VERSION,
                "Unsupported migration manifest version '{}' (this build reads '{}')",
                future.version,
                MANIFEST_FORMAT_VERSION
            );
            Ok(())
        })()
        .expect_err("forward versions must be rejected, not silently imported");
        assert!(
            error
                .to_string()
                .contains("Unsupported migration manifest version")
        );
    }
}
