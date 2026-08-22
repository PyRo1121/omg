//! Request handlers for the daemon

use std::path::Path;
use std::sync::Arc;

use anyhow::Context;
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Quota, RateLimiter};

use super::cache::PackageCache;
use super::index::PackageIndex;
use super::protocol::{
    DetailedPackageInfo, ExplicitResult, HealthStatus, Request, RequestId, Response,
    ResponseResult, SearchResult, SecurityAuditResult, UpdateEntry, Vulnerability, error_codes,
};
use crate::core::metrics::GLOBAL_METRICS;
use crate::core::security::{AuditEventType, AuditSeverity, audit_log};
use crate::package_managers::{PackageManager, get_package_manager};
#[cfg(feature = "arch")]
use crate::package_managers::{alpm_worker::AlpmWorker, search_detailed};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};

// Constants for package source strings to avoid repeated allocations
const SOURCE_APT: &str = "apt";
const SOURCE_OFFICIAL: &str = "official";
#[cfg(feature = "arch")]
const SOURCE_AUR: &str = "aur";
const PING_RESPONSE: &str = "pong";
const CACHE_CLEARED_MSG: &str = "cleared";
const DAEMON_INFO_BACKEND_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
#[cfg(feature = "arch")]
const DAEMON_INFO_AUR_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

enum SystemBackendAccess {
    Isolated,
    Production {
        #[cfg(feature = "arch")]
        alpm_worker: AlpmWorker,
    },
}

impl SystemBackendAccess {
    fn production() -> Self {
        Self::Production {
            #[cfg(feature = "arch")]
            alpm_worker: AlpmWorker::new(),
        }
    }

    fn is_production(&self) -> bool {
        matches!(self, Self::Production { .. })
    }
}

/// Daemon state shared across handlers.
///
/// Fields are visible only to the daemon subtree (`server`, worker tasks);
/// external consumers go through `DaemonState::new` and IPC responses.
pub struct DaemonState {
    pub(super) cache: PackageCache,
    pub(super) persistent: super::db::PersistentCache,
    pub(super) package_manager: Arc<dyn PackageManager>,
    pub(super) index: Arc<PackageIndex>,
    system_backends: SystemBackendAccess,
    pub(super) runtime_versions: Arc<RwLock<Vec<(String, String)>>>,
    pub(super) rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    pub(super) start_time: std::time::Instant,
    background_worker_failures: AtomicU64,
}

impl DaemonState {
    /// Whether the package index has no entries. Intended for tests and
    /// health reporting that must not reach into private state.
    pub fn index_is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub fn new() -> anyhow::Result<Self> {
        let data_dir = crate::core::paths::daemon_data_dir();
        let persistent = Self::open_persistent_cache(&data_dir)?;
        let index = PackageIndex::new().with_context(|| {
            "Failed to build package index. Ensure package databases are synced (run 'omg sync')."
        })?;

        let package_manager = get_package_manager()?;
        Ok(Self::from_index(
            persistent,
            index,
            package_manager,
            SystemBackendAccess::production(),
        ))
    }

    /// Initialize daemon handlers with explicit isolated dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when the persistent cache cannot be opened at `data_dir`.
    pub fn new_isolated(
        data_dir: &Path,
        index: PackageIndex,
        package_manager: Arc<dyn PackageManager>,
    ) -> anyhow::Result<Self> {
        let persistent = Self::open_persistent_cache(data_dir)?;
        Ok(Self::from_index(
            persistent,
            index,
            package_manager,
            SystemBackendAccess::Isolated,
        ))
    }

    fn open_persistent_cache(data_dir: &Path) -> anyhow::Result<super::db::PersistentCache> {
        tracing::info!("Initializing daemon data directory: {:?}", data_dir);

        super::db::PersistentCache::new(data_dir).with_context(|| {
            format!(
                "Failed to initialize persistent cache at {}. \
                 Check permissions and disk space.",
                data_dir.display()
            )
        })
    }

    fn from_index(
        persistent: super::db::PersistentCache,
        index: PackageIndex,
        package_manager: Arc<dyn PackageManager>,
        system_backends: SystemBackendAccess,
    ) -> Self {
        tracing::info!("Package index loaded: {} packages", index.len());

        let cache = PackageCache::default();

        match persistent.get_status() {
            Ok(Some(status)) => {
                cache.update_status(Arc::new(status));
                tracing::debug!("Pre-warmed status cache from persistent storage");
            }
            Ok(None) => {}
            Err(error) => {
                tracing::warn!("Failed to load persisted status cache: {error}");
            }
        }

        let quota = Quota::per_second(crate::core::safe_ops::nonzero_u32_or_default(100, 1))
            .allow_burst(crate::core::safe_ops::nonzero_u32_or_default(200, 1));
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        tracing::info!("Using package manager: {}", package_manager.name());

        // Pre-warm Debian package cache if on Debian/Ubuntu
        #[cfg(any(feature = "debian", feature = "debian-pure"))]
        if system_backends.is_production() {
            tracing::info!("Pre-warming Debian package cache...");
            let start = std::time::Instant::now();

            // Load the full index
            if let Err(error) = crate::package_managers::debian_db::ensure_index_loaded() {
                tracing::warn!("Failed to pre-warm Debian cache: {error}");
            } else {
                tracing::info!("Debian cache pre-warmed in {:?}", start.elapsed());
            }
        }

        Self {
            cache,
            persistent,
            package_manager,
            index: Arc::new(index),
            system_backends,
            runtime_versions: Arc::new(RwLock::new(Vec::new())),
            rate_limiter,
            start_time: std::time::Instant::now(),
            background_worker_failures: AtomicU64::new(0),
        }
    }

    /// Record one unexpected termination of the singleton status worker.
    pub(super) fn inc_background_worker_failures(&self) {
        self.background_worker_failures
            .fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub(super) fn background_worker_failures(&self) -> u64 {
        self.background_worker_failures.load(Ordering::Relaxed)
    }
}

// NOTE: DaemonState does not implement Default because initialization can fail.
// Use DaemonState::new() which returns Result<Self, anyhow::Error> to handle errors properly.

/// Handle an incoming request
#[tracing::instrument(skip(state), fields(request_type = %request.variant_name()))]
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
            result: ResponseResult::Ping(PING_RESPONSE.to_string()),
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
                result: ResponseResult::Message(CACHE_CLEARED_MSG.to_string()),
            }
        }
        Request::Metrics { id } => handle_metrics(id),
        Request::Suggest { id, query, limit } => handle_suggest(state, id, query, limit).await,
        Request::Batch { id, requests } => handle_batch(state, id, requests).await,
        Request::DebianSearch { id, query, limit } => {
            handle_debian_search(state, id, query, limit).await
        }
        Request::Health { id } => handle_health(&state, id),
        Request::ListUpdates { id } => handle_list_updates(state, id).await,
    }
}

/// Handle Debian search request
#[tracing::instrument(skip(state), fields(query_len = query.len()))]
async fn handle_debian_search(
    state: Arc<DaemonState>,
    id: RequestId,
    query: String,
    limit: Option<usize>,
) -> Response {
    // METRICS: Track search requests
    GLOBAL_METRICS.inc_search_requests();

    let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT).min(MAX_SEARCH_LIMIT);

    // Check cache first (Arc clone is cheap - just pointer copy)
    if let Some(cached) = state.cache.get_debian(&query) {
        // METRICS: Cache hit
        GLOBAL_METRICS.inc_cache_hits();
        return Response::Success {
            id,
            result: ResponseResult::DebianSearch(cached.iter().take(limit).cloned().collect()),
        };
    }

    // METRICS: Cache miss - will perform search
    GLOBAL_METRICS.inc_cache_misses();

    // The daemon index is the authoritative, already-loaded package catalog.
    // Searching the package database again here duplicated I/O and made request
    // behavior depend on process-global environment state.
    // Like `handle_search`, run the (potentially large) index scan on the
    // blocking pool instead of the executor thread.
    let state_clone = Arc::clone(&state);
    let query_for_task = query.clone();
    let searched =
        tokio::task::spawn_blocking(move || state_clone.index.search(&query_for_task, limit)).await;

    let mut results = match searched {
        Ok(results) => results,
        Err(error) => return internal_error(id, format!("Debian search task failed: {error}")),
    };
    for package in &mut results {
        package.source = SOURCE_APT.to_string();
    }
    let results = Arc::new(results);
    state.cache.insert_debian_arc(query, Arc::clone(&results));
    Response::Success {
        id,
        result: ResponseResult::DebianSearch(Arc::unwrap_or_clone(results)),
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
/// Default number of suggestions returned
const DEFAULT_SUGGEST_LIMIT: usize = 10;
/// Maximum number of suggestions returned
const MAX_SUGGEST_LIMIT: usize = 50;
/// Concurrency for vulnerability scanning
const SCAN_CONCURRENCY: usize = 32;
/// Cache size threshold for "degraded" health status
const HEALTH_DEGRADED_CACHE_THRESHOLD: usize = 50_000;
/// Cache size threshold for "unhealthy" health status
const HEALTH_UNHEALTHY_CACHE_THRESHOLD: usize = 100_000;
/// Failed request threshold for "unhealthy" health status
const HEALTH_UNHEALTHY_FAILURES_THRESHOLD: u64 = 1000;

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
        cache_hits: snapshot.cache_hits,
        cache_misses: snapshot.cache_misses,
        search_requests: snapshot.search_requests,
        info_requests: snapshot.info_requests,
        status_requests: snapshot.status_requests,
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

    let limit = limit
        .unwrap_or(DEFAULT_SUGGEST_LIMIT)
        .min(MAX_SUGGEST_LIMIT);
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

    // SECURITY: Reject nested batch requests to prevent recursion DoS
    if requests.iter().any(|r| matches!(r, Request::Batch { .. })) {
        return Response::Error {
            id,
            code: error_codes::INVALID_PARAMS,
            message: "Nested batch requests are not allowed".to_string(),
        };
    }

    // SECURITY: Limit expensive operations per batch to prevent resource exhaustion
    // SecurityAudit spawns 32 concurrent scans per request, so limit to 5 per batch
    let security_audit_count = requests
        .iter()
        .filter(|r| matches!(r, Request::SecurityAudit { .. }))
        .count();
    if security_audit_count > 5 {
        let msg =
            format!("Too many SecurityAudit requests in batch: {security_audit_count} (max 5)");
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

    // Process requests concurrently with a limit to prevent DoS.
    // NOTE: each sub-request flows through `handle_request`, so it consumes
    // one global rate-limiter token and increments request metrics; a
    // max-size batch therefore burns half of the global burst budget by
    // design.
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
        result: ResponseResult::Batch(responses),
    }
}

/// Handle search request
#[tracing::instrument(skip(state), fields(query_len = query.len()))]
async fn handle_search(
    state: Arc<DaemonState>,
    id: RequestId,
    query: String,
    limit: Option<usize>,
) -> Response {
    // METRICS: Track search requests
    GLOBAL_METRICS.inc_search_requests();

    // SECURITY: Validate search query to prevent injection attacks
    // Allow more flexible search queries but limit length
    if query.len() > MAX_QUERY_LENGTH {
        return validation_error(
            id,
            format!("Search query too long (max {MAX_QUERY_LENGTH} characters)"),
        );
    }

    let limit = limit.unwrap_or(DEFAULT_SEARCH_LIMIT).min(MAX_SEARCH_LIMIT); // Cap limit to prevent resource exhaustion

    // Check cache first (Arc clone is cheap - just pointer copy)
    if let Some(cached) = state.cache.get(&query) {
        // METRICS: Cache hit
        GLOBAL_METRICS.inc_cache_hits();
        let total = cached.len();
        let packages: Vec<_> = cached.iter().take(limit).cloned().collect();
        return Response::Success {
            id,
            result: ResponseResult::Search(SearchResult { packages, total }),
        };
    }

    // METRICS: Cache miss - will perform search
    GLOBAL_METRICS.inc_cache_misses();

    // 1. Instant Official Search (Sub-millisecond)
    // Run in blocking task to avoid stalling the async runtime during heavy search
    // Cache the full result set (up to MAX_SEARCH_LIMIT) so subsequent requests
    // with different limits are served correctly from cache.
    let state_clone = Arc::clone(&state);
    let query_arc: Arc<str> = Arc::from(query.as_str());
    let query_for_cache = query;

    let official =
        tokio::task::spawn_blocking(move || state_clone.index.search(&query_arc, MAX_SEARCH_LIMIT))
            .await;

    let official = match official {
        Ok(res) => res,
        Err(e) => return internal_error(id, format!("Search task failed: {e}")),
    };

    // Cache the full result set; serve truncated views per request limit
    let official = Arc::new(official);
    let total = official.len();
    state
        .cache
        .insert_arc(query_for_cache, Arc::clone(&official));

    let packages: Vec<_> = official.iter().take(limit).cloned().collect();

    Response::Success {
        id,
        result: ResponseResult::Search(SearchResult { packages, total }),
    }
}

/// Handle info request
#[tracing::instrument(skip(state))]
async fn handle_info(state: Arc<DaemonState>, id: RequestId, package: String) -> Response {
    // METRICS: Track info requests
    GLOBAL_METRICS.inc_info_requests();

    // SECURITY: Validate package name to prevent command injection
    if let Err(e) = crate::core::security::validate_package_name(&package) {
        return validation_error(id, format!("Invalid package name: {e}"));
    }

    // 1. Check cache first (Arc clone is cheap - just pointer copy)
    if let Some(cached) = state.cache.get_info(&package) {
        // METRICS: Cache hit
        GLOBAL_METRICS.inc_cache_hits();
        return Response::Success {
            id,
            result: ResponseResult::Info(Arc::unwrap_or_clone(cached)),
        };
    }

    if state.cache.is_info_miss(&package) {
        return not_found_error(id, format!("Package not found: {package}"));
    }

    // METRICS: Cache miss - will fetch package info
    GLOBAL_METRICS.inc_cache_misses();

    // 2. Try official index (Instant hash lookup)
    if let Some(pkg) = state.index.get(&package) {
        // Clone once, then use Arc for cheap sharing
        let info = Arc::new(pkg);
        state.cache.insert_info_arc(Arc::clone(&info));
        return Response::Success {
            id,
            result: ResponseResult::Info(Arc::unwrap_or_clone(info)),
        };
    }

    // 3. Try Package Manager Backend. Only a genuine `Ok(None)` falls through
    // to the next source; backend errors and timeouts are reported explicitly
    // instead of being silently converted into "package not found".
    match tokio::time::timeout(
        DAEMON_INFO_BACKEND_TIMEOUT,
        state.package_manager.info(&package),
    )
    .await
    {
        Ok(Ok(Some(info))) => {
            let detailed = Arc::new(DetailedPackageInfo {
                name: info.name,
                #[allow(
                    clippy::implicit_clone,
                    reason = "the package version type varies by backend feature"
                )]
                version: info.version.to_string(),
                description: info.description,
                url: String::new(), // info.url not in Package struct currently
                size: 0,
                download_size: 0,
                repo: String::new(),
                depends: Vec::new(),
                licenses: Vec::new(),
                source: SOURCE_OFFICIAL.to_string(),
            });
            state.cache.insert_info_arc(Arc::clone(&detailed));
            return Response::Success {
                id,
                result: ResponseResult::Info(Arc::unwrap_or_clone(detailed)),
            };
        }
        Ok(Ok(None)) => {}
        Ok(Err(error)) => {
            tracing::warn!("Info backend error for {package}: {error:#}");
            return internal_error(id, format!("Info backend failed for {package}: {error}"));
        }
        Err(_) => {
            tracing::warn!(
                "Info backend timed out after {DAEMON_INFO_BACKEND_TIMEOUT:?} for {package}"
            );
            return internal_error(
                id,
                format!(
                    "Info backend timed out after {} seconds",
                    DAEMON_INFO_BACKEND_TIMEOUT.as_secs()
                ),
            );
        }
    }

    // 4. Try AUR (arch only). AUR is best-effort for availability, but a
    // failed or timed-out lookup is surfaced loudly (mirroring step 3's
    // backend semantics) instead of silently masquerading as "not found".
    // Only a genuine miss (empty results or no exact name match) falls
    // through to the negative cache.
    #[cfg(feature = "arch")]
    if state.system_backends.is_production() && state.package_manager.name() == "pacman" {
        match tokio::time::timeout(DAEMON_INFO_AUR_TIMEOUT, search_detailed(&package)).await {
            Ok(Ok(details)) => {
                if let Some(pkg) = details.into_iter().find(|p| p.name == package) {
                    let detailed = Arc::new(DetailedPackageInfo {
                        name: pkg.name,
                        version: pkg.version.clone(),
                        description: pkg.description.unwrap_or_default(),
                        url: pkg.url.unwrap_or_default(),
                        size: 0,
                        download_size: 0,
                        repo: SOURCE_AUR.to_string(),
                        depends: pkg.depends.unwrap_or_default(),
                        licenses: pkg.license.unwrap_or_default(),
                        source: SOURCE_AUR.to_string(),
                    });

                    state.cache.insert_info_arc(Arc::clone(&detailed));
                    return Response::Success {
                        id,
                        result: ResponseResult::Info(Arc::unwrap_or_clone(detailed)),
                    };
                }
            }
            Ok(Err(error)) => {
                tracing::warn!("AUR lookup failed for {package}: {error:#}");
                return internal_error(id, format!("AUR lookup failed for {package}: {error}"));
            }
            Err(_) => {
                tracing::warn!(
                    "AUR lookup timed out after {DAEMON_INFO_AUR_TIMEOUT:?} for {package}"
                );
                return internal_error(
                    id,
                    format!(
                        "AUR lookup timed out after {} seconds",
                        DAEMON_INFO_AUR_TIMEOUT.as_secs()
                    ),
                );
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

/// Query native status counts for a production package-manager backend.
/// Dispatch a native backend query, keeping every feature-gated arm in one
/// place. Disabled branches collapse to a single canonical error so the two
/// query shapes can never drift apart. Adding a native backend means adding
/// an arm here and updating the feature gates below.
macro_rules! native_backend_query {
    ($pm_name:expr, $debian:expr, $debian_pure:expr, $arch:expr) => {
        match $pm_name {
            "apt" => {
                #[cfg(feature = "debian")]
                {
                    $debian
                }
                #[cfg(not(feature = "debian"))]
                {
                    Err(backend_disabled("Debian"))
                }
            }
            "apt-pure" => {
                #[cfg(any(feature = "debian", feature = "debian-pure"))]
                {
                    $debian_pure
                }
                #[cfg(not(any(feature = "debian", feature = "debian-pure")))]
                {
                    Err(backend_disabled("Debian"))
                }
            }
            "pacman" => {
                #[cfg(feature = "arch")]
                {
                    $arch
                }
                #[cfg(not(feature = "arch"))]
                {
                    Err(backend_disabled("Arch"))
                }
            }
            other => Err(anyhow::anyhow!("Unsupported package manager: {other}")),
        }
    };
}

#[cold]
fn backend_disabled(backend: &str) -> anyhow::Error {
    anyhow::anyhow!("{backend} backend disabled")
}

/// Query native status counts for a production package-manager backend.
pub(crate) fn system_status_for_backend(
    pm_name: &str,
) -> anyhow::Result<(usize, usize, usize, usize)> {
    native_backend_query!(
        pm_name,
        crate::package_managers::apt_get_system_status(),
        crate::package_managers::debian_db::get_counts_fast(),
        crate::package_managers::get_system_status()
    )
}

pub(crate) fn explicit_packages_for_backend(pm_name: &str) -> anyhow::Result<Vec<String>> {
    native_backend_query!(
        pm_name,
        crate::package_managers::apt_list_explicit(),
        crate::package_managers::debian_db::list_explicit_fast(),
        crate::package_managers::list_explicit_fast()
    )
}

/// Handle status request
#[tracing::instrument(skip(state))]
async fn handle_status(state: Arc<DaemonState>, id: RequestId) -> Response {
    // METRICS: Track status requests
    GLOBAL_METRICS.inc_status_requests();

    // 1. Check MEMORY cache first (instant - sub-microsecond, Arc clone is cheap)
    if let Some(cached) = state.cache.get_status() {
        // METRICS: Cache hit (memory)
        GLOBAL_METRICS.inc_cache_hits();
        return Response::Success {
            id,
            result: ResponseResult::Status(Arc::unwrap_or_clone(cached)),
        };
    }

    // 2. Check persistent cache (disk - slower)
    // Runs in blocking thread to avoid stalling async runtime
    let state_clone = Arc::clone(&state);
    let cached_result =
        tokio::task::spawn_blocking(move || state_clone.persistent.get_status()).await;

    match cached_result {
        Ok(Ok(Some(cached))) => {
            // METRICS: Cache hit (persistent)
            GLOBAL_METRICS.inc_cache_hits();
            // Promote to memory cache for next hit (Arc avoids clone)
            let cached_arc = Arc::new(cached);
            state.cache.update_status(Arc::clone(&cached_arc));
            return Response::Success {
                id,
                result: ResponseResult::Status(Arc::unwrap_or_clone(cached_arc)),
            };
        }
        Ok(Ok(None)) => {}
        Ok(Err(error)) => {
            tracing::warn!("Failed to read persisted status cache: {error}");
        }
        Err(error) => {
            tracing::warn!("Status cache task failed: {error}");
        }
    }

    // METRICS: Cache miss - need to query system
    GLOBAL_METRICS.inc_cache_misses();

    // 3. Query the selected backend. Production uses the optimized native
    // status paths; dependency-injected states stay behind the package-manager
    // interface and never access host package databases.
    let status_result = if state.system_backends.is_production() {
        let state_clone = Arc::clone(&state);
        match tokio::task::spawn_blocking(move || {
            system_status_for_backend(state_clone.package_manager.name())
        })
        .await
        {
            Ok(result) => result,
            Err(error) => return internal_error(id, format!("Status task panicked: {error}")),
        }
    } else {
        state.package_manager.get_status(false).await
    };

    match status_result {
        Ok((total, explicit, orphans, updates)) => {
            let (res, cacheable) = super::status_policy::status_snapshot(
                total,
                explicit,
                orphans,
                updates,
                state
                    .runtime_versions
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone(),
                None,
            );
            debug_assert!(
                !cacheable,
                "package-count snapshots without a vulnerability scan must not be cached"
            );

            Response::Success {
                id,
                result: ResponseResult::Status(res),
            }
        }
        Err(error) => internal_error(id, format!("Failed to get system status: {error}")),
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
        let vulns = match res {
            Ok(vulns) => vulns,
            Err(error) => {
                return internal_error(
                    id,
                    format!("Failed to scan package {name} for vulnerabilities: {error}"),
                );
            }
        };
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
    // Arc clone is cheap - just pointer copy
    if let Some(cached) = state.cache.get_explicit() {
        return Response::Success {
            id,
            result: ResponseResult::Explicit(ExplicitResult {
                packages: Arc::unwrap_or_clone(cached),
            }),
        };
    }

    let state_clone = Arc::clone(&state);
    let packages_result = tokio::task::spawn_blocking(move || {
        explicit_packages_for_backend(state_clone.package_manager.name())
    })
    .await;

    match packages_result {
        Ok(Ok(packages)) => {
            let packages_arc = Arc::new(packages);
            state.cache.update_explicit_arc(Arc::clone(&packages_arc));
            Response::Success {
                id,
                result: ResponseResult::Explicit(ExplicitResult {
                    packages: Arc::unwrap_or_clone(packages_arc),
                }),
            }
        }
        Ok(Err(e)) => internal_error(id, format!("Failed to list explicit packages: {e}")),
        Err(e) => internal_error(id, format!("List explicit task panicked: {e}")),
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
        explicit_packages_for_backend(state_clone.package_manager.name())
            .map(|packages| packages.len())
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
        Ok(Err(e)) => internal_error(id, format!("Failed to count explicit packages: {e}")),
        Err(e) => internal_error(id, format!("Explicit count task panicked: {e}")),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Handler Dispatch Helpers - Reduce Boilerplate
// ═══════════════════════════════════════════════════════════════════════════════

/// Create a validation error response with logging and metrics
#[cold]
#[inline(never)]
fn validation_error(id: RequestId, message: impl Into<String>) -> Response {
    let msg = message.into();
    audit_log(
        AuditEventType::PolicyViolation,
        AuditSeverity::Warning,
        "daemon_handler",
        &msg,
    );
    GLOBAL_METRICS.inc_validation_failures();
    GLOBAL_METRICS.inc_requests_failed();
    Response::Error {
        id,
        code: error_codes::INVALID_PARAMS,
        message: msg,
    }
}

/// Resident memory of the daemon process in MiB, parsed from procfs.
/// `/proc/self/status` is a kernel virtual file served from memory (no
/// device I/O), so the synchronous read is effectively free.
fn process_rss_mb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024)
}

fn handle_health(state: &Arc<DaemonState>, id: RequestId) -> Response {
    let uptime_seconds = state.start_time.elapsed().as_secs();
    let cache_size = state.cache.stats().size;
    let metrics = GLOBAL_METRICS.snapshot();
    let active_connections = metrics.active_connections;

    let memory_usage_mb = process_rss_mb().unwrap_or(0);

    let status = if cache_size > HEALTH_UNHEALTHY_CACHE_THRESHOLD
        || metrics.requests_failed > HEALTH_UNHEALTHY_FAILURES_THRESHOLD
    {
        "unhealthy".to_string()
    } else if cache_size > HEALTH_DEGRADED_CACHE_THRESHOLD {
        "degraded".to_string()
    } else {
        "healthy".to_string()
    };

    Response::Success {
        id,
        result: ResponseResult::Health(HealthStatus {
            status,
            uptime_seconds,
            memory_usage_mb,
            cache_size,
            active_connections,
            background_worker_failures: state.background_worker_failures(),
        }),
    }
}

/// Names listed in pacman.conf `IgnorePkg` must never appear in update lists.
///
/// The hot ALPM worker owns a bare `Alpm` handle without
/// `configure_package_filters`, so unlike the CLI path
/// (`alpm_ops::get_update_list`, which relies on `should_ignore()` at the
/// source) it would report ignored packages as updatable. This replicates
/// the name-level filter on the daemon side so both surfaces agree.
/// Group-based ignores (`IgnoreGroup`) need ALPM group membership and are
/// still only applied on the direct CLI path.
#[cfg(feature = "arch")]
fn filter_ignored_updates<T>(
    updates: Vec<T>,
    ignored_pkgs: &[String],
    name: impl Fn(&T) -> &str,
) -> Vec<T> {
    if ignored_pkgs.is_empty() {
        return updates;
    }
    let ignored: std::collections::HashSet<&str> =
        ignored_pkgs.iter().map(String::as_str).collect();
    updates
        .into_iter()
        .filter(|update| !ignored.contains(name(update)))
        .collect()
}

/// Handle list updates request using the hot ALPM worker (zero ALPM init overhead)
async fn handle_list_updates(state: Arc<DaemonState>, id: RequestId) -> Response {
    #[cfg(feature = "arch")]
    let updates_result = match &state.system_backends {
        SystemBackendAccess::Production { alpm_worker } => alpm_worker.list_updates().await,
        SystemBackendAccess::Isolated => state.package_manager.list_updates().await,
    };

    #[cfg(not(feature = "arch"))]
    let updates_result = state.package_manager.list_updates().await;

    // `mut` is only consumed by the arch-gated IgnorePkg filter below.
    #[cfg_attr(not(feature = "arch"), allow(unused_mut))]
    match updates_result {
        Ok(mut updates) => {
            // PARITY: apply the same IgnorePkg filter as the CLI path; a
            // pacman.conf parse failure is an error, mirroring
            // `alpm_ops::get_update_list`.
            #[cfg(feature = "arch")]
            if state.system_backends.is_production() && state.package_manager.name() == "pacman" {
                match crate::core::pacman_conf::PacmanConfig::parse(
                    crate::core::paths::pacman_conf_path(),
                ) {
                    Ok(pacman_config) => {
                        updates =
                            filter_ignored_updates(updates, &pacman_config.ignore_pkg, |update| {
                                update.name.as_str()
                            });
                    }
                    Err(error) => {
                        return internal_error(
                            id,
                            format!("Failed to load update filters from pacman.conf: {error}"),
                        );
                    }
                }
            }

            Response::Success {
                id,
                result: ResponseResult::ListUpdates(
                    updates
                        .into_iter()
                        .map(|update| UpdateEntry {
                            name: update.name,
                            old_version: update.old_version,
                            new_version: update.new_version,
                            repo: update.repo,
                        })
                        .collect(),
                ),
            }
        }
        Err(error) => internal_error(id, format!("Failed to list updates: {error}")),
    }
}

/// Create an internal error response with metrics
#[cold]
#[inline(never)]
fn internal_error(id: RequestId, message: impl Into<String>) -> Response {
    GLOBAL_METRICS.inc_requests_failed();
    Response::Error {
        id,
        code: error_codes::INTERNAL_ERROR,
        message: message.into(),
    }
}

/// Create a not found error response
#[cold]
#[inline(never)]
fn not_found_error(id: RequestId, message: impl Into<String>) -> Response {
    Response::Error {
        id,
        code: error_codes::PACKAGE_NOT_FOUND,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_rejects_unknown_package_manager() {
        let error = system_status_for_backend("homebrew")
            .expect_err("unknown backends must not invent a healthy status");
        assert!(
            error.to_string().contains("Unsupported package manager"),
            "got: {error}"
        );
    }

    #[test]
    #[cfg(not(any(feature = "debian", feature = "debian-pure")))]
    fn apt_pure_status_without_debian_fails() {
        let error = system_status_for_backend("apt-pure")
            .expect_err("apt-pure without a Debian backend must not invent counts");
        assert!(
            error.to_string().contains("Debian backend disabled"),
            "got: {error}"
        );
    }

    #[test]
    fn explicit_rejects_unknown_package_manager() {
        let error = explicit_packages_for_backend("homebrew")
            .expect_err("unknown backends must not invent an empty explicit list");
        assert!(
            error.to_string().contains("Unsupported package manager"),
            "got: {error}"
        );
    }

    #[test]
    #[cfg(not(any(feature = "debian", feature = "debian-pure")))]
    fn apt_pure_explicit_without_debian_fails() {
        let error = explicit_packages_for_backend("apt-pure")
            .expect_err("apt-pure without a Debian backend must not invent an empty explicit list");
        assert!(
            error.to_string().contains("Debian backend disabled"),
            "got: {error}"
        );
    }

    #[cfg(feature = "arch")]
    fn update_entry(name: &str) -> UpdateEntry {
        UpdateEntry {
            name: name.to_string(),
            old_version: "1.0".to_string(),
            new_version: "2.0".to_string(),
            repo: "core".to_string(),
        }
    }

    /// PARITY: the daemon's update list must exclude pacman.conf IgnorePkg
    /// names exactly like the CLI path (`should_ignore()` at the ALPM source).
    #[test]
    #[cfg(feature = "arch")]
    fn update_list_filters_ignored_packages_like_the_cli() {
        let ignored = vec!["linux".to_string(), "linux-lts".to_string()];
        let updates = vec![
            update_entry("linux"),
            update_entry("firefox"),
            update_entry("linux-lts"),
            update_entry("git"),
        ];

        let kept = filter_ignored_updates(updates, &ignored, |update| update.name.as_str());
        let names: Vec<&str> = kept.iter().map(|update| update.name.as_str()).collect();
        assert_eq!(names, ["firefox", "git"]);
    }

    #[test]
    #[cfg(feature = "arch")]
    fn update_list_without_ignores_is_passed_through() {
        let updates = vec![update_entry("firefox"), update_entry("linux")];
        let kept = filter_ignored_updates(updates, &[], |update| update.name.as_str());
        assert_eq!(kept.len(), 2);
    }

    #[test]
    fn health_reports_resident_memory_from_procfs() {
        let rss_mb = process_rss_mb().expect("VmRSS must be readable on Linux");
        assert!(rss_mb > 0, "a running test process has non-zero RSS");
    }
}
