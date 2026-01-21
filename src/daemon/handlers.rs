//! Request handlers for the daemon

use std::sync::Arc;

use super::cache::PackageCache;
use super::index::PackageIndex;
use super::protocol::{
    DetailedPackageInfo, ExplicitResult, Request, RequestId, Response, ResponseResult,
    SearchResult, SecurityAuditResult, StatusResult, Vulnerability, error_codes,
};
use crate::package_managers::{AurClient, PackageManager, get_package_manager};
#[cfg(feature = "arch")]
use crate::package_managers::{alpm_worker::AlpmWorker, search_detailed};
use parking_lot::RwLock;

/// Daemon state shared across handlers
pub struct DaemonState {
    pub cache: PackageCache,
    pub persistent: super::db::PersistentCache,
    pub package_manager: Box<dyn PackageManager>,
    #[cfg(feature = "arch")]
    pub aur: AurClient,
    #[cfg(feature = "arch")]
    pub alpm_worker: AlpmWorker,
    pub index: Arc<PackageIndex>,
    pub runtime_versions: Arc<RwLock<Vec<(String, String)>>>,
}

impl DaemonState {
    #[must_use]
    #[allow(clippy::expect_used)]
    pub fn new() -> Self {
        let data_dir = crate::core::paths::daemon_data_dir();
        let persistent = super::db::PersistentCache::new(&data_dir).expect("daemon requires redb");

        // Use cached index loading for instant startup
        let index = PackageIndex::new_with_cache(&persistent).unwrap_or_else(|e| {
            tracing::warn!("Failed to load cached index: {e}, building fresh");
            PackageIndex::new().expect("daemon requires index")
        });

        let cache = PackageCache::default();

        // Pre-warm caches from persistent storage for instant first queries
        if let Ok(Some(status)) = persistent.get_status() {
            cache.update_status(status);
            tracing::debug!("Pre-warmed status cache from persistent storage");
        }

        Self {
            cache,
            persistent,
            package_manager: get_package_manager(),
            #[cfg(feature = "arch")]
            aur: AurClient::new(),
            #[cfg(feature = "arch")]
            alpm_worker: AlpmWorker::new(),
            index: Arc::new(index),
            runtime_versions: Arc::new(RwLock::new(Vec::new())),
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
    match request {
        Request::Search { id, query, limit } => handle_search(&state, id, query, limit),
        Request::Info { id, package } => handle_info(state, id, package).await,
        Request::Ping { id } => Response::Success {
            id,
            result: ResponseResult::Ping(String::from("pong")),
        },
        Request::Status { id } => handle_status(&state, id),
        Request::Explicit { id } => handle_list_explicit(&state, id),
        Request::ExplicitCount { id } => handle_explicit_count(&state, id),
        Request::SecurityAudit { id } => handle_security_audit(state, id).await,
        Request::CacheStats { id } => {
            let stats = state.cache.stats();
            Response::Success {
                id,
                result: ResponseResult::CacheStats {
                    size: stats.size,
                    max_size: stats.max_size,
                },
            }
        }
        Request::CacheClear { id } => {
            state.cache.clear();
            Response::Success {
                id,
                result: ResponseResult::Message("cleared".to_string()),
            }
        }
        Request::Batch { id, requests } => handle_batch(state, id, *requests).await,
    }
}

/// Handle batch requests - process multiple requests in parallel
async fn handle_batch(state: Arc<DaemonState>, id: RequestId, requests: Vec<Request>) -> Response {
    use futures::stream::{self, StreamExt};

    // Process requests concurrently with a limit to prevent DoS
    let responses: Vec<_> = stream::iter(requests)
        .map(|req| {
            let state = Arc::clone(&state);
            async move { handle_request(state, req).await }
        })
        .buffer_unordered(16) // Limit concurrency to 16
        .collect()
        .await;

    Response::Success {
        id,
        result: ResponseResult::Batch(Box::new(responses)),
    }
}

/// Handle search request
#[inline]
fn handle_search(
    state: &Arc<DaemonState>,
    id: RequestId,
    query: String,
    limit: Option<usize>,
) -> Response {
    let limit = limit.unwrap_or(50);

    // Check cache first
    if let Some(cached) = state.cache.get(&query) {
        let packages: Vec<_> = cached.into_iter().take(limit).collect();
        let total = packages.len();
        return Response::Success {
            id,
            result: ResponseResult::Search(SearchResult { packages, total }),
        };
    }

    // 1. Instant Official Search (Sub-millisecond)
    let official = state.index.search(&query, limit);

    // Cache results and return (clone only for cache, return original)
    let total = official.len();
    state.cache.insert(query, official.clone());

    Response::Success {
        id,
        result: ResponseResult::Search(SearchResult {
            packages: official,
            total,
        }),
    }
}

/// Handle info request
#[inline]
async fn handle_info(state: Arc<DaemonState>, id: RequestId, package: String) -> Response {
    // 1. Check cache first
    if let Some(cached) = state.cache.get_info(&package) {
        return Response::Success {
            id,
            result: ResponseResult::Info(cached),
        };
    }

    if state.cache.is_info_miss(&package) {
        return Response::Error {
            id,
            code: error_codes::PACKAGE_NOT_FOUND,
            message: format!("Package not found: {package}"),
        };
    }

    // 2. Try official index (Instant hash lookup)
    if let Some(pkg) = state.index.get(&package) {
        state.cache.insert_info(pkg.clone());
        return Response::Success {
            id,
            result: ResponseResult::Info(pkg),
        };
    }

    // 3. Try Package Manager Backend
    if let Ok(Some(info)) = state.package_manager.info(&package).await {
        let detailed = DetailedPackageInfo {
            name: info.name,
            version: info.version.to_string(),
            description: info.description,
            url: String::new(), // info.url not in Package struct currently
            size: 0,
            download_size: 0,
            repo: String::new(),
            depends: Vec::new(),
            licenses: Vec::new(),
            source: "official".to_string(),
        };
        state.cache.insert_info(detailed.clone());
        return Response::Success {
            id,
            result: ResponseResult::Info(detailed),
        };
    }

    // 4. Try AUR (arch only)
    #[cfg(feature = "arch")]
    if state.package_manager.name() == "pacman" {
        if let Ok(details) = search_detailed(&package).await
            && let Some(pkg) = details.into_iter().find(|p| p.name == package)
        {
            let detailed = DetailedPackageInfo {
                name: pkg.name,
                version: pkg.version.clone(),
                description: pkg.description.unwrap_or_default(),
                url: pkg.url.unwrap_or_default(),
                size: 0,
                download_size: 0,
                repo: "aur".to_string(),
                depends: pkg.depends.unwrap_or_default(),
                licenses: pkg.license.unwrap_or_default(),
                source: "aur".to_string(),
            };

            state.cache.insert_info(detailed.clone());
            return Response::Success {
                id,
                result: ResponseResult::Info(detailed),
            };
        }
    }

    state.cache.insert_info_miss(&package);

    Response::Error {
        id,
        code: error_codes::PACKAGE_NOT_FOUND,
        message: format!("Package not found: {package}"),
    }
}

/// Handle status request
fn handle_status(state: &Arc<DaemonState>, id: RequestId) -> Response {
    // 1. Check MEMORY cache first (instant - sub-microsecond)
    if let Some(cached) = state.cache.get_status() {
        return Response::Success {
            id,
            result: ResponseResult::Status(cached),
        };
    }

    // 2. Check persistent cache (disk - slower)
    if let Ok(Some(cached)) = state.persistent.get_status() {
        // Promote to memory cache for next hit
        state.cache.update_status(cached.clone());
        return Response::Success {
            id,
            result: ResponseResult::Status(cached),
        };
    }

    // 3. Query system backends
    let pm_name = state.package_manager.name();

    let status = if pm_name == "apt" {
        #[cfg(feature = "debian")]
        {
            crate::package_managers::apt_get_system_status()
        }
        #[cfg(not(feature = "debian"))]
        {
            Err(anyhow::anyhow!("Debian backend disabled"))
        }
    } else if pm_name == "pacman" {
        #[cfg(feature = "arch")]
        {
            crate::package_managers::get_system_status()
        }
        #[cfg(not(feature = "arch"))]
        {
            Err(anyhow::anyhow!("Arch backend disabled"))
        }
    } else {
        Err(anyhow::anyhow!("Unsupported package manager: {}", pm_name))
    };

    match status {
        Ok((total, explicit, orphans, updates)) => {
            let res = StatusResult {
                total_packages: total,
                explicit_packages: explicit,
                orphan_packages: orphans,
                updates_available: updates,
                security_vulnerabilities: 0,
                runtime_versions: state.runtime_versions.read().clone(),
            };

            let _ = state.persistent.set_status(&res);
            state.cache.update_status(res.clone());

            Response::Success {
                id,
                result: ResponseResult::Status(res),
            }
        }
        Err(e) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to get system status: {e}"),
        },
    }
}

/// Handle security audit request
async fn handle_security_audit(state: Arc<DaemonState>, id: RequestId) -> Response {
    use crate::core::security::vulnerability::VulnerabilityScanner;

    let scanner = VulnerabilityScanner::new();
    let installed = state.package_manager.list_installed().await;

    let installed = match installed {
        Ok(pkgs) => pkgs,
        Err(e) => {
            return Response::Error {
                id,
                code: error_codes::INTERNAL_ERROR,
                message: format!("Failed to list packages: {e}"),
            };
        }
    };

    let mut vulnerabilities = Vec::new();
    let mut total_vulns = 0;
    let mut high_severity = 0;

    let scanner = Arc::new(scanner);

    // Use bounded concurrency instead of limiting the count
    use futures::stream::{self, StreamExt};

    let mut stream = stream::iter(installed)
        .map(|pkg| {
            let scanner = scanner.clone();
            async move {
                let name = pkg.name.clone();
                let version = pkg.version.clone();
                let res = scanner.scan_package(&name, &version).await;
                (name, res)
            }
        })
        .buffer_unordered(32); // Scan up to 32 packages concurrently

    while let Some((name, res)) = stream.next().await {
        if let Ok(vulns) = res
            && !vulns.is_empty()
        {
            let mapped: Vec<Vulnerability> = vulns
                .into_iter()
                .map(|v| {
                    if let Some(score_str) = &v.score
                        && let Ok(score) = score_str.parse::<f32>()
                        && score >= 7.0
                    {
                        high_severity += 1;
                    }
                    Vulnerability {
                        id: v.id,
                        summary: v.summary,
                        score: v.score,
                    }
                })
                .collect();
            total_vulns += mapped.len();
            vulnerabilities.push((name, mapped));
        }
    }

    let result = SecurityAuditResult {
        total_vulnerabilities: total_vulns,
        high_severity,
        vulnerabilities,
    };

    Response::Success {
        id,
        result: ResponseResult::SecurityAudit(result),
    }
}

/// Handle list explicit request
fn handle_list_explicit(state: &Arc<DaemonState>, id: RequestId) -> Response {
    if let Some(cached) = state.cache.get_explicit() {
        return Response::Success {
            id,
            result: ResponseResult::Explicit(ExplicitResult { packages: cached }),
        };
    }

    let pm_name = state.package_manager.name();
    let packages = if pm_name == "apt" {
        #[cfg(feature = "debian")]
        {
            crate::package_managers::apt_list_explicit()
        }
        #[cfg(not(feature = "debian"))]
        {
            Err(anyhow::anyhow!("Debian backend disabled"))
        }
    } else if pm_name == "pacman" {
        #[cfg(feature = "arch")]
        {
            crate::package_managers::list_explicit_fast()
        }
        #[cfg(not(feature = "arch"))]
        {
            Err(anyhow::anyhow!("Arch backend disabled"))
        }
    } else {
        Err(anyhow::anyhow!("Unsupported package manager: {}", pm_name))
    };

    match packages {
        Ok(packages) => {
            state.cache.update_explicit(packages.clone());
            Response::Success {
                id,
                result: ResponseResult::Explicit(ExplicitResult { packages }),
            }
        }
        Err(e) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to list explicit packages: {e}"),
        },
    }
}

/// Handle explicit package count request
fn handle_explicit_count(state: &Arc<DaemonState>, id: RequestId) -> Response {
    if let Some(cached) = state.cache.get_explicit_count() {
        return Response::Success {
            id,
            result: ResponseResult::ExplicitCount(cached),
        };
    }

    let pm_name = state.package_manager.name();
    let count = if pm_name == "apt" {
        #[cfg(feature = "debian")]
        {
            crate::package_managers::apt_list_explicit().map(|packages| packages.len())
        }
        #[cfg(not(feature = "debian"))]
        {
            Err(anyhow::anyhow!("Debian backend disabled"))
        }
    } else if pm_name == "pacman" {
        #[cfg(feature = "arch")]
        {
            crate::package_managers::list_explicit_fast().map(|packages| packages.len())
        }
        #[cfg(not(feature = "arch"))]
        {
            Err(anyhow::anyhow!("Arch backend disabled"))
        }
    } else {
        Err(anyhow::anyhow!("Unsupported package manager: {}", pm_name))
    };

    match count {
        Ok(count) => {
            state.cache.update_explicit_count(count);
            Response::Success {
                id,
                result: ResponseResult::ExplicitCount(count),
            }
        }
        Err(e) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to count explicit packages: {e}"),
        },
    }
}
