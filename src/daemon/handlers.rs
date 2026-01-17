//! Request handlers for the daemon

use std::sync::Arc;

use super::cache::PackageCache;
use super::index::PackageIndex;
use super::protocol::{
    DetailedPackageInfo, ExplicitResult, PackageInfo, Request, RequestId, Response, ResponseResult,
    SearchResult, SecurityAuditResult, StatusResult, Vulnerability, error_codes,
};
#[cfg(feature = "debian")]
use crate::core::env::distro::is_debian_like;

#[cfg(feature = "arch")]
use crate::package_managers::{
    AurClient, OfficialPackageManager, alpm_worker::AlpmWorker, list_installed_fast,
    search_detailed,
};
use parking_lot::RwLock;

#[cfg(feature = "debian")]
use crate::package_managers::{
    apt_get_sync_pkg_info, apt_get_system_status, apt_list_explicit, apt_list_installed_fast,
};

fn use_debian_backend() -> bool {
    #[cfg(feature = "debian")]
    {
        return is_debian_like();
    }

    #[cfg(not(feature = "debian"))]
    {
        false
    }
}

/// Daemon state shared across handlers
pub struct DaemonState {
    pub cache: PackageCache,
    pub persistent: super::db::PersistentCache,
    #[cfg(feature = "arch")]
    pub pacman: OfficialPackageManager,
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

        Self {
            cache: PackageCache::default(),
            persistent,
            #[cfg(feature = "arch")]
            pacman: OfficialPackageManager::new(),
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
        Request::Search { id, query, limit } => handle_search(state, id, query, limit).await,
        Request::Info { id, package } => handle_info(state, id, package).await,
        Request::Ping { id } => Response::Success {
            id,
            result: ResponseResult::Ping("pong".to_string()),
        },
        Request::Status { id } => handle_status(&state, id),
        Request::Explicit { id } => handle_list_explicit(&state, id),
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
        Request::Batch { id, requests } => handle_batch(state, id, requests).await,
    }
}

/// Handle batch requests - process multiple requests in parallel
async fn handle_batch(state: Arc<DaemonState>, id: RequestId, requests: Vec<Request>) -> Response {
    use futures::future::join_all;

    // Process all requests concurrently
    let futures: Vec<_> = requests
        .into_iter()
        .map(|req| {
            let state = Arc::clone(&state);
            async move { Box::pin(handle_request(state, req)).await }
        })
        .collect();

    let responses = join_all(futures).await;

    Response::Success {
        id,
        result: ResponseResult::Batch(responses),
    }
}

/// Handle search request
async fn handle_search(
    state: Arc<DaemonState>,
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

    if use_debian_backend() {
        let total = official.len();
        state.cache.insert(query, official.clone());
        return Response::Success {
            id,
            result: ResponseResult::Search(SearchResult {
                packages: official,
                total,
            }),
        };
    }

    // 2. Conditional AUR Search (Network Bound) - Arch only
    // Only search AUR if official results are low, to keep speed for common packages
    let mut aur = Vec::new();
    #[cfg(feature = "arch")]
    if official.len() < 5
        && let Ok(aur_pkgs) = state.aur.search(&query).await
    {
        for pkg in aur_pkgs {
            aur.push(PackageInfo {
                name: pkg.name,
                version: pkg.version.to_string(),
                description: pkg.description,
                source: "aur".to_string(),
            });
        }
    }

    // Combined results
    let mut packages: Vec<PackageInfo> = Vec::with_capacity(official.len() + aur.len());
    packages.extend(official);
    packages.extend(aur);

    // Cache the results
    state.cache.insert(query, packages.clone());

    let total = packages.len();
    let packages: Vec<_> = packages.into_iter().take(limit).collect();

    Response::Success {
        id,
        result: ResponseResult::Search(SearchResult { packages, total }),
    }
}

/// Handle info request
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

    if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            if let Ok(Some(info)) = apt_get_sync_pkg_info(&package) {
                let detailed = DetailedPackageInfo {
                    name: info.name,
                    version: info.version.to_string(),
                    description: info.description,
                    url: info.url.unwrap_or_default(),
                    size: info.size,
                    download_size: info.download_size.unwrap_or(0),
                    repo: info.repo,
                    depends: info.depends,
                    licenses: info.licenses,
                    source: "official".to_string(),
                };
                state.cache.insert_info(detailed.clone());
                return Response::Success {
                    id,
                    result: ResponseResult::Info(detailed),
                };
            }
        }
    } else {
        // 3. Try AUR (arch only)
        #[cfg(feature = "arch")]
        {
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
    if let Ok(Some(cached)) = state.persistent.get_status() {
        return Response::Success {
            id,
            result: ResponseResult::Status(cached),
        };
    }

    if let Some(cached) = state.cache.get_status() {
        return Response::Success {
            id,
            result: ResponseResult::Status(cached),
        };
    }

    #[cfg(feature = "arch")]
    let status = if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            apt_get_system_status()
        }
        #[cfg(not(feature = "debian"))]
        {
            Err(anyhow::anyhow!("Debian backend disabled"))
        }
    } else {
        use crate::package_managers::get_system_status;
        get_system_status()
    };

    #[cfg(not(feature = "arch"))]
    let status = if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            apt_get_system_status()
        }
        #[cfg(not(feature = "debian"))]
        {
            Err(anyhow::anyhow!("No package manager backend available"))
        }
    } else {
        Err(anyhow::anyhow!("Arch backend disabled"))
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
async fn handle_security_audit(_state: Arc<DaemonState>, id: RequestId) -> Response {
    use crate::core::security::vulnerability::VulnerabilityScanner;

    let scanner = VulnerabilityScanner::new();
    let installed: Result<Vec<crate::package_managers::types::LocalPackage>, anyhow::Error> =
        if use_debian_backend() {
            #[cfg(feature = "debian")]
            {
                apt_list_installed_fast()
            }
            #[cfg(not(feature = "debian"))]
            {
                Err(anyhow::anyhow!("Debian backend disabled"))
            }
        } else {
            #[cfg(feature = "arch")]
            {
                list_installed_fast()
            }
            #[cfg(not(feature = "arch"))]
            {
                Err(anyhow::anyhow!("Arch backend disabled"))
            }
        };

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
    let mut set = tokio::task::JoinSet::new();

    for pkg in installed.iter().take(20) {
        let scanner = scanner.clone();
        let name = pkg.name.clone();
        let version = pkg.version.clone();
        set.spawn(async move {
            let res = scanner.scan_package(&name, &version).await;
            (name, res)
        });
    }

    while let Some(res) = set.join_next().await {
        if let Ok((name, Ok(vulns))) = res
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

    #[cfg(feature = "arch")]
    let packages = if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            apt_list_explicit()
        }
        #[cfg(not(feature = "debian"))]
        {
            Err(anyhow::anyhow!("Debian backend disabled"))
        }
    } else {
        use crate::package_managers::list_explicit_fast;
        list_explicit_fast()
    };

    #[cfg(not(feature = "arch"))]
    let packages = if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            apt_list_explicit()
        }
        #[cfg(not(feature = "debian"))]
        {
            Err(anyhow::anyhow!("No package manager backend available"))
        }
    } else {
        Err(anyhow::anyhow!("Arch backend disabled"))
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
