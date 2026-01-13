//! Request handlers for the daemon

use std::sync::Arc;

use super::cache::PackageCache;
use super::protocol::*;
use crate::package_managers::{
    alpm_worker::AlpmWorker, list_installed_fast, search_detailed, ArchPackageManager, AurClient,
    PackageManager,
};

/// Daemon state shared across handlers
pub struct DaemonState {
    pub cache: PackageCache,
    pub persistent: super::db::PersistentCache,
    pub pacman: ArchPackageManager,
    pub aur: AurClient,
    pub alpm_worker: AlpmWorker,
}

impl DaemonState {
    pub fn new() -> Self {
        let data_dir = directories::ProjectDirs::from("com", "omg", "omg")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("/var/lib/omg"));

        let db_path = data_dir.join("cache.mdb");

        DaemonState {
            cache: PackageCache::default(),
            persistent: super::db::PersistentCache::new(&db_path).expect("Failed to init LMDB"),
            pacman: ArchPackageManager::new(),
            aur: AurClient::new(),
            alpm_worker: AlpmWorker::new(),
        }
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle an incoming request
pub async fn handle_request(state: Arc<DaemonState>, request: Request) -> Response {
    tracing::debug!("Handling request: {} (id={})", request.method, request.id);

    match request.method.as_str() {
        "search" => handle_search(state, request).await,
        "info" => handle_info(state, request).await,
        "ping" => handle_ping(request),
        "status" => handle_status(state, request).await,
        "security_audit" => handle_security_audit(state, request).await,
        "cache.stats" => handle_cache_stats(state, request),
        "cache.clear" => handle_cache_clear(state, request),
        _ => Response::error(
            request.id,
            error_codes::METHOD_NOT_FOUND,
            format!("Unknown method: {}", request.method),
        ),
    }
}

/// Handle search request
async fn handle_search(state: Arc<DaemonState>, request: Request) -> Response {
    // Parse params
    let params: SearchParams = match serde_json::from_value(request.params) {
        Ok(p) => p,
        Err(e) => {
            return Response::error(
                request.id,
                error_codes::INVALID_PARAMS,
                format!("Invalid params: {}", e),
            );
        }
    };

    let limit = params.limit.unwrap_or(50);

    // Check cache first
    if let Some(cached) = state.cache.get(&params.query) {
        tracing::debug!("Cache hit for query: {}", params.query);
        let packages: Vec<_> = cached.into_iter().take(limit).collect();
        let total = packages.len();
        return Response::success(request.id, SearchResult { packages, total });
    }

    tracing::debug!("Cache miss for query: {}", params.query);

    // Search official repos
    let official = state.pacman.search(&params.query).await.unwrap_or_default();

    // Search AUR
    let aur = state.aur.search(&params.query).await.unwrap_or_default();

    // Combine results
    let mut packages: Vec<PackageInfo> = Vec::new();

    for pkg in official {
        packages.push(PackageInfo {
            name: pkg.name,
            version: pkg.version,
            description: pkg.description,
            source: "official".to_string(),
        });
    }

    for pkg in aur {
        packages.push(PackageInfo {
            name: pkg.name,
            version: pkg.version,
            description: pkg.description,
            source: "aur".to_string(),
        });
    }

    // Cache the results
    state.cache.insert(params.query.clone(), packages.clone());

    // Apply limit
    let packages: Vec<_> = packages.into_iter().take(limit).collect();
    let total = packages.len();

    Response::success(request.id, SearchResult { packages, total })
}

/// Handle info request
async fn handle_info(state: Arc<DaemonState>, request: Request) -> Response {
    #[derive(serde::Deserialize)]
    struct InfoParams {
        package: String,
    }

    let params: InfoParams = match serde_json::from_value(request.params) {
        Ok(p) => p,
        Err(e) => {
            return Response::error(
                request.id,
                error_codes::INVALID_PARAMS,
                format!("Invalid params: {}", e),
            );
        }
    };

    // 1. Check cache first
    if let Some(cached) = state.cache.get_info(&params.package) {
        tracing::debug!("Detailed info cache hit for: {}", params.package);
        return Response::success(request.id, cached);
    }

    tracing::debug!("Detailed info cache miss for: {}", params.package);

    // 2. Try official repos first
    let pkg_name = params.package.clone();
    let info = state.alpm_worker.get_info(pkg_name).await;

    if let Ok(Some(pkg)) = info {
        let detailed = DetailedPackageInfo {
            name: pkg.name,
            version: pkg.version,
            description: pkg.description,
            url: pkg.url,
            size: pkg.size,
            download_size: pkg.download_size,
            repo: pkg.repo,
            depends: pkg.depends,
            licenses: pkg.licenses,
            source: "official".to_string(),
        };

        // Cache the result
        state.cache.insert_info(detailed.clone());
        return Response::success(request.id, detailed);
    }

    // 3. Try AUR
    if let Ok(details) = search_detailed(&params.package).await {
        if let Some(pkg) = details.into_iter().find(|p| p.name == params.package) {
            let detailed = DetailedPackageInfo {
                name: pkg.name,
                version: pkg.version,
                description: pkg.description.unwrap_or_default(),
                url: pkg.url.unwrap_or_default(),
                size: 0,
                download_size: 0,
                repo: "aur".to_string(),
                depends: pkg.depends.unwrap_or_default(),
                licenses: pkg.license.unwrap_or_default(),
                source: "aur".to_string(),
            };

            // Cache the result
            state.cache.insert_info(detailed.clone());
            return Response::success(request.id, detailed);
        }
    }

    Response::error(
        request.id,
        error_codes::PACKAGE_NOT_FOUND,
        format!("Package not found: {}", params.package),
    )
}

/// Handle status request
async fn handle_status(state: Arc<DaemonState>, request: Request) -> Response {
    use crate::package_managers::get_system_status;

    // 1. Check persistent LMDB cache first (survives restart)
    if let Ok(Some(cached)) = state.persistent.get_status() {
        tracing::debug!("System status persistent cache hit");
        return Response::success(request.id, cached);
    }

    // 2. Check in-memory cache
    if let Some(cached) = state.cache.get_status() {
        tracing::debug!("System status in-memory cache hit");
        return Response::success(request.id, cached);
    }

    tracing::debug!("System status cache miss, computing...");

    // 3. Fallback to computation
    match get_system_status() {
        Ok((total, explicit, orphans, updates)) => {
            let res = StatusResult {
                total_packages: total,
                explicit_packages: explicit,
                orphan_packages: orphans,
                updates_available: updates,
                security_vulnerabilities: 0, // Initial value, will be populated by worker
            };

            // 4. Update both caches
            let _ = state.persistent.set_status(res.clone());
            state.cache.update_status(res.clone());

            Response::success(request.id, res)
        }
        Err(e) => Response::error(
            request.id,
            error_codes::INTERNAL_ERROR,
            format!("Failed to get system status: {}", e),
        ),
    }
}

/// Handle security audit request
async fn handle_security_audit(_state: Arc<DaemonState>, request: Request) -> Response {
    use crate::core::security::vulnerability::VulnerabilityScanner;

    let scanner = VulnerabilityScanner::new();
    let installed = match list_installed_fast() {
        Ok(pkgs) => pkgs,
        Err(e) => {
            return Response::error(
                request.id,
                error_codes::INTERNAL_ERROR,
                format!("Failed to list packages: {}", e),
            )
        }
    };

    let mut vulnerabilities = Vec::new();
    let mut total_vulns = 0;
    let mut high_severity = 0;

    // For MVP/Demo, we only scan a sample because a full scan of 1000+ packages
    // would hit OSV rate limits or take too long without batching.
    // In production, the background worker would pre-fetch this.
    for pkg in installed.iter().take(20) {
        if let Ok(vulns) = scanner.scan_package(&pkg.name, &pkg.version).await {
            if !vulns.is_empty() {
                let mapped: Vec<Vulnerability> = vulns
                    .into_iter()
                    .map(|v| {
                        // Check if high severity (score >= 7.0)
                        if let Some(score_str) = &v.score {
                            if let Ok(score) = score_str.parse::<f32>() {
                                if score >= 7.0 {
                                    high_severity += 1;
                                }
                            }
                        }
                        Vulnerability {
                            id: v.id,
                            summary: v.summary,
                            score: v.score,
                        }
                    })
                    .collect();
                total_vulns += mapped.len();
                vulnerabilities.push((pkg.name.clone(), mapped));
            }
        }
    }

    let result = SecurityAuditResult {
        total_vulnerabilities: total_vulns,
        high_severity,
        vulnerabilities,
    };

    Response::success(request.id, result)
}

/// Handle ping request
fn handle_ping(request: Request) -> Response {
    Response::success(request.id, "pong")
}

/// Handle cache stats request
fn handle_cache_stats(state: Arc<DaemonState>, request: Request) -> Response {
    let cache_stats = state.cache.stats();
    Response::success(
        request.id,
        serde_json::json!({
            "size": cache_stats.size,
            "max_size": cache_stats.max_size,
        }),
    )
}

/// Handle cache clear request
fn handle_cache_clear(state: Arc<DaemonState>, request: Request) -> Response {
    state.cache.clear();
    Response::success(request.id, "cleared")
}
