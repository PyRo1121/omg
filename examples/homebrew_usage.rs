//! Example usage of the Homebrew package manager backend
//!
//! This example demonstrates the pure Rust Homebrew implementation
//! with direct filesystem and API access.

use anyhow::Result;
use omg_lib::core::Package;
use omg_lib::package_managers::types::UpdateInfo;
use omg_lib::package_managers::{HomebrewPackageManager, PackageManager};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the package manager
    let pm = HomebrewPackageManager::new();
    println!("Package Manager: {}\n", pm.name());

    // Example 1: Search for packages
    println!("=== Searching for 'python' ===");
    let results: Vec<Package> = pm.search("python").await?;
    println!("Found {} packages:", results.len());
    for (i, pkg) in results.iter().take(5).enumerate() {
        println!(
            "  {}. {} v{} - {}",
            i + 1,
            pkg.name,
            pkg.version,
            pkg.description
        );
    }
    println!();

    // Example 2: List installed packages
    println!("=== Installed Packages ===");
    let installed: Vec<Package> = pm.list_installed().await?;
    println!("Total installed: {}", installed.len());
    for (i, pkg) in installed.iter().take(10).enumerate() {
        println!(
            "  {}. {} v{}{}",
            i + 1,
            pkg.name,
            pkg.version,
            if pkg.installed { " ✓" } else { "" }
        );
    }
    println!();

    // Example 3: Get package information
    println!("=== Package Info: wget ===");
    if let Some(pkg) = pm.info("wget").await? {
        println!("Name: {}", pkg.name);
        println!("Version: {}", pkg.version);
        println!("Description: {}", pkg.description);
        println!("Installed: {}", if pkg.installed { "Yes" } else { "No" });
    } else {
        println!("Package not found");
    }
    println!();

    // Example 4: Check if package is installed
    println!("=== Installation Checks ===");
    let packages = vec!["git", "wget", "curl", "python3"];
    for pkg in packages {
        let installed = pm.is_installed(pkg).await;
        println!("  {} {}", pkg, if installed { "✓" } else { "✗" });
    }
    println!();

    // Example 5: Get system status
    println!("=== System Status ===");
    let (total, explicit, orphans, updates) = pm.get_status(false).await?;
    println!("Total packages: {}", total);
    println!("Explicitly installed: {}", explicit);
    println!("Orphaned packages: {}", orphans);
    println!("Available updates: {}", updates);
    println!();

    // Example 6: List explicit packages
    println!("=== Explicitly Installed ===");
    let explicit: Vec<String> = pm.list_explicit().await?;
    println!("Count: {}", explicit.len());
    for (i, pkg) in explicit.iter().take(10).enumerate() {
        println!("  {}. {}", i + 1, pkg);
    }
    println!();

    // Example 7: Check for updates
    println!("=== Available Updates ===");
    let updates: Vec<UpdateInfo> = pm.list_updates().await?;
    if updates.is_empty() {
        println!("System is up to date! ✓");
    } else {
        println!("Found {} updates:", updates.len());
        for (i, update) in updates.iter().take(5).enumerate() {
            println!(
                "  {}. {} ({} -> {})",
                i + 1,
                update.name,
                update.old_version,
                update.new_version
            );
        }
    }
    println!();

    // Example 8: Performance benchmark
    println!("=== Performance Benchmark ===");
    use std::time::Instant;

    let start = Instant::now();
    let _ = pm.search("python").await?;
    let search_time = start.elapsed();
    println!("Search: {:?}", search_time);

    let start = Instant::now();
    let _ = pm.list_installed().await?;
    let list_time = start.elapsed();
    println!("List installed: {:?}", list_time);

    let start = Instant::now();
    let _ = pm.is_installed("git").await;
    let check_time = start.elapsed();
    println!("Check installed: {:?}", check_time);

    Ok(())
}
