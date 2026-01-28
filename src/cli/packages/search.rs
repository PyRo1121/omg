use std::io::Write;

use anyhow::Result;
use serde::Serialize;

use crate::cli::style;
use crate::core::Package;
use crate::core::client::DaemonClient;
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
        let json_str = serde_json::to_string_pretty(&display_packages)
            .unwrap_or_else(|_| "[]".to_string());
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

/// Attempts to execute package search in a new Tokio runtime from a synchronous context.
///
/// This function provides a bridge between synchronous CLI code and the async `search()` function.
/// It's designed to be called from contexts where no Tokio runtime exists yet, allowing the
/// async search to execute synchronously by creating a temporary runtime.
///
/// # Return Value
///
/// Returns `Ok(true)` if the search was successfully executed in this function.
/// Returns `Ok(false)` if the caller should use the async path instead. This occurs when:
///   - The query fails basic validation (too long or contains control characters)
///   - A Tokio runtime already exists in the current context
///
/// Returns `Err(_)` if the search itself fails (validation errors beyond basic checks,
/// runtime creation failures, or search execution errors).
///
/// # When to Use This Function
///
/// Use this function when:
/// - You're in a synchronous context (no `.await` available)
/// - You want to attempt a synchronous search before falling back to async
/// - You need to integrate async search into legacy sync code
///
/// **Do NOT use this function when:**
/// - You're already inside an async context (just call `search().await` directly)
/// - You already have a Tokio runtime handle available
///
/// # Why Return `Result<bool>` Instead of Executing Directly?
///
/// The boolean return value allows callers to implement a fallback strategy:
/// ```rust,ignore
/// // Try sync path first, fall back to async if needed
/// if search_sync_cli(query, detailed, interactive, no_aur)? {
///     // Search completed successfully
/// } else {
///     // Already in async context, use async path
///     search(query, detailed, interactive, no_aur).await?;
/// }
/// ```
///
/// # Runtime Creation Safety
///
/// This function creates a new current-thread runtime using
/// `tokio::runtime::Builder::new_current_thread()`. This approach:
///
/// - Prevents nested runtime panics (the most common issue with `block_on`)
/// - Avoids the "cannot drop a runtime in a context where blocking is prohibited" error
/// - Is safe to call from any synchronous context
///
/// The check for existing runtime (`Handle::try_current()`) is critical because:
/// - Attempting to create a runtime inside an existing runtime causes a panic
/// - Calling `block_on` from within an async context can cause deadlocks
/// - Returning `false` allows the caller to use the existing async context properly
///
/// # Arguments
///
/// * `query` - The search query string (max 100 characters, no control chars)
/// * `detailed` - Whether to perform detailed search (includes AUR details on Arch)
/// * `interactive` - Interactive mode flag (currently unused but reserved for future use)
/// * `no_aur` - Skip AUR search and only search official repositories
///
/// # Errors
///
/// This function will return an error if:
/// - Tokio runtime creation fails
/// - The underlying `search()` function returns an error
/// - I/O operations fail during search execution
///
/// # Example
///
/// ```rust,ignore
/// use crate::cli::packages::search::search_sync_cli;
///
/// fn main() -> anyhow::Result<()> {
///     let query = "firefox";
///     let detailed = false;
///     let interactive = false;
///     let no_aur = false; // Include AUR results
///
///     // Try synchronous execution
///     if search_sync_cli(query, detailed, interactive, no_aur)? {
///         println!("Search completed synchronously");
///     } else {
///         println!("Fallback to async path needed");
///     }
///     Ok(())
/// }
/// ```
pub fn search_sync_cli(
    query: &str,
    detailed: bool,
    interactive: bool,
    no_aur: bool,
) -> Result<bool> {
    // Perform basic validation before attempting any async operations
    // These checks are lightweight and can fail fast without runtime overhead
    if query.len() > 100 || query.chars().any(char::is_control) {
        return Ok(false);
    }

    // Detect if we're already inside a Tokio runtime context
    //
    // This check is critical for preventing runtime panics:
    // - Creating a new runtime inside an existing one causes panic: "cannot create runtime
    //   from within another runtime"
    // - Using block_on inside an async context can lead to deadlocks
    //
    // When a runtime exists, we return Ok(false) to signal the caller that they should
    // use the async path directly (call search().await instead of this sync wrapper)
    if tokio::runtime::Handle::try_current().is_ok() {
        return Ok(false);
    }

    // No runtime exists, so we can safely create one and execute the async search
    // synchronously. Using current_thread runtime because:
    // - Search operations are I/O bound, not CPU bound
    // - Avoids overhead of multi-threaded runtime for single operations
    // - Prevents potential thread-locale issues with package manager commands
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    // Execute the async search function and block until completion
    rt.block_on(search(query, detailed, interactive, no_aur))?;

    // Return true to indicate successful completion via the sync path
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
