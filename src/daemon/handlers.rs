//! Request handlers for the daemon

use std::num::NonZeroU32;
use std::sync::Arc;

use anyhow::Context;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};

use super::cache::PackageCache;
use super::index::PackageIndex;
use super::protocol::{
    DetailedPackageInfo, ExplicitResult, PackageInfo, Request, RequestId, Response, ResponseResult,
    SearchResult, SecurityAuditResult, StatusResult, Vulnerability, error_codes,
};
use crate::core::metrics::GLOBAL_METRICS;
use crate::core::security::{AuditEventType, AuditSeverity, audit_log};
#[cfg(feature = "arch")]
use crate::package_managers::AurClient;
use crate::package_managers::{PackageManager, get_package_manager};
#[cfg(feature = "arch")]
use crate::package_managers::{alpm_worker::AlpmWorker, search_detailed};
use parking_lot::RwLock;

/// Daemon state shared across handlers
pub struct DaemonState {
    pub cache: PackageCache,
    pub persistent: super::db::PersistentCache,
    pub package_manager: Box<dyn PackageManager>,
    #[cfg(feature = "arch")]
    pub aur: crate::package_managers::AurClient,
    #[cfg(feature = "arch")]
    pub alpm_worker: AlpmWorker,
    pub index: Arc<PackageIndex>,
    pub runtime_versions: Arc<RwLock<Vec<(String, String)>>>,
    pub rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl DaemonState {
    /// Create a new daemon state
    ///
    /// # Errors
    /// Returns an error if the persistent cache or package index cannot be initialized.
    pub fn new() -> anyhow::Result<Self> {
        let data_dir = crate::core::paths::daemon_data_dir();
        let persistent = super::db::PersistentCache::new(&data_dir)
            .context("Failed to initialize persistent cache (redb)")?;

        // Use cached index loading for instant startup
        let index = PackageIndex::new_with_cache(&persistent)
            .or_else(|e| {
                tracing::warn!("Failed to load cached index: {e}, building fresh");
                PackageIndex::new()
            })
            .context("Failed to build package index")?;

        let cache = PackageCache::default();

        // Pre-warm caches from persistent storage for instant first queries
        if let Ok(Some(status)) = persistent.get_status() {
            cache.update_status(status);
            tracing::debug!("Pre-warmed status cache from persistent storage");
        }

        // Rate limit: 100 requests per second with burst of 200
        let quota = Quota::per_second(NonZeroU32::new(100).unwrap())
            .allow_burst(NonZeroU32::new(200).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        Ok(Self {
            cache,
            persistent,
            package_manager: get_package_manager(),
            #[cfg(feature = "arch")]
            aur: AurClient::new(),
            #[cfg(feature = "arch")]
            alpm_worker: AlpmWorker::new(),
            index: Arc::new(index),
            runtime_versions: Arc::new(RwLock::new(Vec::new())),
            rate_limiter,
        })
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new().expect("Failed to initialize DaemonState")
    }
}

/// Handle an incoming request
pub async fn handle_request(state: Arc<DaemonState>, request: Request) -> Response {
    // METRICS: Track total requests handled
    GLOBAL_METRICS.inc_requests_total();

    // SECURITY: Enforce rate limiting
    if state.rate_limiter.check().is_err() {
        tracing::warn!("Rate limit exceeded for request");
        audit_log(
            AuditEventType::PolicyViolation,
            AuditSeverity::Warning,
            "daemon_handler",
            "Global rate limit exceeded",
        );
        GLOBAL_METRICS.inc_rate_limit_hits();
        GLOBAL_METRICS.inc_requests_failed();
        return Response::Error {
            id: request.id(),
            code: error_codes::RATE_LIMITED,
            message: "Rate limit exceeded. Please slow down.".to_string(),
        };
    }

    match request {
        Request::Search { id, query, limit } => handle_search(state, id, query, limit).await,
        Request::Info { id, package } => handle_info(state, id, package).await,
        Request::Ping { id } => Response::Success {
            id,
            result: ResponseResult::Ping(String::from("pong")),
        },
        Request::Status { id } => handle_status(state, id).await,
        Request::Explicit { id } => handle_list_explicit(state, id).await,
        Request::ExplicitCount { id } => handle_explicit_count(state, id).await,
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
        Request::Metrics { id } => handle_metrics(id),
        Request::Suggest { id, query, limit } => handle_suggest(state, id, query, limit).await,
        Request::Batch { id, requests } => handle_batch(state, id, *requests).await,
        Request::DebianSearch { id, query, limit } => {
            handle_debian_search(state, id, query, limit).await
        }
    }
}

/// Handle Debian search request
async fn handle_debian_search(
    state: Arc<DaemonState>,
    id: RequestId,
    query: String,
    limit: Option<usize>,
) -> Response {
    let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT).min(MAX_SEARCH_LIMIT);

    // Check cache first
    if let Some(cached) = state.cache.get_debian(&query) {
        return Response::Success {
            id,
            result: ResponseResult::DebianSearch(cached.into_iter().take(limit).collect()),
        };
    }

    // Run search in blocking task
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    let query_clone = query.clone();
    let results = tokio::task::spawn_blocking(move || {
        #[cfg(any(feature = "debian", feature = "debian-pure"))]
        {
            crate::package_managers::apt_search_fast(&query_clone).map(|pkgs| {
                pkgs.into_iter()
                    .map(|p| PackageInfo {
                        name: p.name,
                        #[allow(clippy::implicit_clone)]
                        version: p.version.to_string(),
                        description: p.description,
                        source: "apt".to_string(),
                    })
                    .collect::<Vec<PackageInfo>>()
            })
        }
        #[cfg(not(any(feature = "debian", feature = "debian-pure")))]
        {
            Err::<Vec<PackageInfo>, _>(anyhow::anyhow!("Debian backend disabled"))
        }
    })
    .await;

    match results {
        Ok(Ok(pkgs)) => {
            state.cache.insert_debian(query, pkgs.clone());
            Response::Success {
                id,
                result: ResponseResult::DebianSearch(pkgs.into_iter().take(limit).collect()),
            }
        }
        Ok(Err(e)) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Debian search failed: {e}"),
        },
        Err(e) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Debian search task panicked: {e}"),
        },
    }
}

/// Maximum number of requests in a batch to prevent `DoS`
const MAX_BATCH_SIZE: usize = 100;
/// Maximum concurrency for batch processing
const BATCH_CONCURRENCY: usize = 16;
/// Maximum length of search query
const MAX_QUERY_LENGTH: usize = 500;
/// Default search limit
const DEFAULT_SEARCH_LIMIT: usize = 50;
/// Maximum search limit
const MAX_SEARCH_LIMIT: usize = 1000;
/// Concurrency for vulnerability scanning
const SCAN_CONCURRENCY: usize = 32;

/// Handle metrics request
fn handle_metrics(id: RequestId) -> Response {
    let snapshot = GLOBAL_METRICS.snapshot();

    // Map internal metrics snapshot to protocol snapshot
    // This decouples the internal representation from the wire format
    let protocol_snapshot = super::protocol::MetricsSnapshot {
        requests_total: snapshot.requests_total,
        requests_failed: snapshot.requests_failed,
        rate_limit_hits: snapshot.rate_limit_hits,
        validation_failures: snapshot.validation_failures,
        active_connections: snapshot.active_connections,
        security_audit_requests: snapshot.security_audit_requests,
        bytes_received: snapshot.bytes_received,
        bytes_sent: snapshot.bytes_sent,
    };

    Response::Success {
        id,
        result: ResponseResult::Metrics(protocol_snapshot),
    }
}

/// Handle suggest request
async fn handle_suggest(
    state: Arc<DaemonState>,
    id: RequestId,
    query: String,
    limit: Option<usize>,
) -> Response {
    // SECURITY: Validate query length
    if query.len() > MAX_QUERY_LENGTH {
        return Response::Error {
            id,
            code: error_codes::INVALID_PARAMS,
            message: "Query too long".to_string(),
        };
    }

    let limit = limit.unwrap_or(10).min(50);
    let state_clone = Arc::clone(&state);

    // Run fuzzy search in blocking thread
    let suggestions =
        tokio::task::spawn_blocking(move || state_clone.index.suggest(&query, limit)).await;

    match suggestions {
        Ok(results) => Response::Success {
            id,
            result: ResponseResult::Suggest(results),
        },
        Err(e) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Suggest task failed: {e}"),
        },
    }
}

/// Handle batch requests - process multiple requests in parallel
async fn handle_batch(state: Arc<DaemonState>, id: RequestId, requests: Vec<Request>) -> Response {
    use futures::stream::{self, StreamExt};

    // SECURITY: Limit batch size to prevent resource exhaustion
    if requests.len() > MAX_BATCH_SIZE {
        let msg = format!(
            "Batch size {} exceeds maximum of {}",
            requests.len(),
            MAX_BATCH_SIZE
        );
        audit_log(
            AuditEventType::PolicyViolation,
            AuditSeverity::Warning,
            "daemon_handler",
            &msg,
        );
        return Response::Error {
            id,
            code: error_codes::INVALID_PARAMS,
            message: msg,
        };
    }

    // Process requests concurrently with a limit to prevent DoS
    let responses: Vec<_> = stream::iter(requests)
        .map(|req| {
            let state = Arc::clone(&state);
            async move { handle_request(state, req).await }
        })
        .buffer_unordered(BATCH_CONCURRENCY) // Limit concurrency
        .collect()
        .await;

    Response::Success {
        id,
        result: ResponseResult::Batch(Box::new(responses)),
    }
}

/// Handle search request
#[inline]
async fn handle_search(
    state: Arc<DaemonState>,
    id: RequestId,
    query: String,
    limit: Option<usize>,
) -> Response {
    // SECURITY: Validate search query to prevent injection attacks
    // Allow more flexible search queries but limit length
    if query.len() > MAX_QUERY_LENGTH {
        let msg = format!("Search query too long (max {MAX_QUERY_LENGTH} characters)");
        audit_log(
            AuditEventType::PolicyViolation,
            AuditSeverity::Warning,
            "daemon_handler",
            &msg,
        );
        GLOBAL_METRICS.inc_validation_failures();
        GLOBAL_METRICS.inc_requests_failed();
        return Response::Error {
            id,
            code: error_codes::INVALID_PARAMS,
            message: msg,
        };
    }

    let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT).min(MAX_SEARCH_LIMIT); // Cap limit to prevent resource exhaustion

    // Check cache first
    if let Some(cached) = state.cache.get(&query) {
        // Avoid intermediate allocation by calculating total from cached length
        let total = cached.len().min(limit);
        let packages: Vec<_> = cached.into_iter().take(limit).collect();
        return Response::Success {
            id,
            result: ResponseResult::Search(SearchResult { packages, total }),
        };
    }

    // 1. Instant Official Search (Sub-millisecond)
    // Run in blocking task to avoid stalling the async runtime during heavy search
    let state_clone = Arc::clone(&state);
    let query_clone = query.clone();

    let official =
        tokio::task::spawn_blocking(move || state_clone.index.search(&query_clone, limit)).await;

    let official = match official {
        Ok(res) => res,
        Err(e) => {
            return Response::Error {
                id,
                code: error_codes::INTERNAL_ERROR,
                message: format!("Search task failed: {e}"),
            };
        }
    };

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
    // SECURITY: Validate package name to prevent command injection
    if let Err(e) = crate::core::security::validate_package_name(&package) {
        let msg = format!("Invalid package name: {e}");
        audit_log(
            AuditEventType::PolicyViolation,
            AuditSeverity::Warning,
            "daemon_handler",
            &msg,
        );
        GLOBAL_METRICS.inc_validation_failures();
        GLOBAL_METRICS.inc_requests_failed();
        return Response::Error {
            id,
            code: error_codes::INVALID_PARAMS,
            message: msg,
        };
    }

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
            #[allow(clippy::implicit_clone)]
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
    if state.package_manager.name() == "pacman"
        && let Ok(details) = search_detailed(&package).await
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

    state.cache.insert_info_miss(&package);

    Response::Error {
        id,
        code: error_codes::PACKAGE_NOT_FOUND,
        message: format!("Package not found: {package}"),
    }
}

/// Handle status request
async fn handle_status(state: Arc<DaemonState>, id: RequestId) -> Response {
    // 1. Check MEMORY cache first (instant - sub-microsecond)
    if let Some(cached) = state.cache.get_status() {
        return Response::Success {
            id,
            result: ResponseResult::Status(cached),
        };
    }

    // 2. Check persistent cache (disk - slower)
    // Runs in blocking thread to avoid stalling async runtime
    let state_clone = Arc::clone(&state);
    let cached_result =
        tokio::task::spawn_blocking(move || state_clone.persistent.get_status()).await;

    if let Ok(Ok(Some(cached))) = cached_result {
        // Promote to memory cache for next hit
        state.cache.update_status(cached.clone());
        return Response::Success {
            id,
            result: ResponseResult::Status(cached),
        };
    }

    // 3. Query system backends (Heavy I/O)
    let state_clone = Arc::clone(&state);
    let status_result = tokio::task::spawn_blocking(move || {
        let pm_name = state_clone.package_manager.name();

        if pm_name == "apt" {
            #[cfg(feature = "debian")]
            {
                crate::package_managers::apt_get_system_status()
            }
            #[cfg(not(feature = "debian"))]
            {
                Err::<(usize, usize, usize, usize), _>(anyhow::anyhow!("Debian backend disabled"))
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
            Err(anyhow::anyhow!("Unsupported package manager: {pm_name}"))
        }
    })
    .await;

    match status_result {
        Ok(Ok((total, explicit, orphans, updates))) => {
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
        Ok(Err(e)) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to get system status: {e}"),
        },
        Err(e) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Status task panicked: {e}"),
        },
    }
}

/// Handle security audit request
async fn handle_security_audit(state: Arc<DaemonState>, id: RequestId) -> Response {
    GLOBAL_METRICS.inc_security_audit_requests();
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

    // OPTIMIZATION: Pre-allocate with expected capacity (assume ~10% hit rate)
    let mut vulnerabilities = Vec::with_capacity(installed.len() / 10);
    let mut total_vulns = 0;
    let mut high_severity = 0;

    let scanner = Arc::new(scanner);

    // Use bounded concurrency instead of limiting the count
    use futures::stream::{self, StreamExt};

    let mut stream = stream::iter(installed)
        .map(|pkg| {
            let scanner = Arc::clone(&scanner); // Use Arc::clone for clarity
            async move {
                // Avoid clones by moving pkg if possible, but here we just need name/version
                let name = pkg.name;
                let version = pkg.version;
                let res = scanner.scan_package(&name, &version).await;
                (name, res)
            }
        })
        .buffer_unordered(SCAN_CONCURRENCY); // Scan up to 32 packages concurrently

    while let Some((name, res)) = stream.next().await {
        let Ok(vulns) = res else { continue };
        if vulns.is_empty() {
            continue;
        }

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

    let result = SecurityAuditResult {
        total_vulnerabilities: total_vulns,
        high_severity,
        vulnerabilities,
    };

    audit_log(
        AuditEventType::SecurityAudit,
        AuditSeverity::Info,
        "daemon_handler",
        &format!(
            "Security audit completed: {total_vulns} vulnerabilities found ({high_severity} high severity)"
        ),
    );

    Response::Success {
        id,
        result: ResponseResult::SecurityAudit(result),
    }
}

/// Handle list explicit request
async fn handle_list_explicit(state: Arc<DaemonState>, id: RequestId) -> Response {
    if let Some(cached) = state.cache.get_explicit() {
        return Response::Success {
            id,
            result: ResponseResult::Explicit(ExplicitResult { packages: cached }),
        };
    }

    let state_clone = Arc::clone(&state);
    let packages_result = tokio::task::spawn_blocking(move || {
        let pm_name = state_clone.package_manager.name();
        if pm_name == "apt" {
            #[cfg(feature = "debian")]
            {
                crate::package_managers::apt_list_explicit()
            }
            #[cfg(not(feature = "debian"))]
            {
                Err::<Vec<String>, _>(anyhow::anyhow!("Debian backend disabled"))
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
            Err(anyhow::anyhow!("Unsupported package manager: {pm_name}"))
        }
    })
    .await;

    match packages_result {
        Ok(Ok(packages)) => {
            state.cache.update_explicit(packages.clone());
            Response::Success {
                id,
                result: ResponseResult::Explicit(ExplicitResult { packages }),
            }
        }
        Ok(Err(e)) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to list explicit packages: {e}"),
        },
        Err(e) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("List explicit task panicked: {e}"),
        },
    }
}

/// Handle explicit package count request
async fn handle_explicit_count(state: Arc<DaemonState>, id: RequestId) -> Response {
    if let Some(cached) = state.cache.get_explicit_count() {
        return Response::Success {
            id,
            result: ResponseResult::ExplicitCount(cached),
        };
    }

    let state_clone = Arc::clone(&state);
    let count_result = tokio::task::spawn_blocking(move || {
        let pm_name = state_clone.package_manager.name();
        if pm_name == "apt" {
            #[cfg(feature = "debian")]
            {
                crate::package_managers::apt_list_explicit().map(|packages| packages.len())
            }
            #[cfg(not(feature = "debian"))]
            {
                Err::<usize, _>(anyhow::anyhow!("Debian backend disabled"))
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
            Err(anyhow::anyhow!("Unsupported package manager: {pm_name}"))
        }
    })
    .await;

    match count_result {
        Ok(Ok(count)) => {
            state.cache.update_explicit_count(count);
            Response::Success {
                id,
                result: ResponseResult::ExplicitCount(count),
            }
        }
        Ok(Err(e)) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to count explicit packages: {e}"),
        },
        Err(e) => Response::Error {
            id,
            code: error_codes::INTERNAL_ERROR,
            message: format!("Explicit count task panicked: {e}"),
        },
    }
}
