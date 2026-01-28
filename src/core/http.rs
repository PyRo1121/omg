//! Shared HTTP client utilities
//!
//! Centralizes reqwest client configuration for connection pooling
//! and consistent timeouts across the codebase.

use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Client;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

static SHARED_CLIENT: LazyLock<Client> =
    LazyLock::new(|| build_client(DEFAULT_TIMEOUT, DEFAULT_CONNECT_TIMEOUT));
static DOWNLOAD_CLIENT: LazyLock<Client> =
    LazyLock::new(|| build_client(DOWNLOAD_TIMEOUT, DOWNLOAD_CONNECT_TIMEOUT));

/// Build HTTP client with standard configuration.
///
/// This function uses `.expect()` because:
/// 1. All configuration values are static and known-valid
/// 2. Building can only fail with TLS backend issues (extremely rare)
/// 3. If this fails, the application cannot function at all (no network = no package manager)
/// 4. Panicking early on startup is better than propagating errors through the entire app
///
/// # Panics
/// Panics if the HTTP client cannot be built, which should only happen with:
/// - Missing TLS certificates (system misconfiguration)
/// - Incompatible TLS backend (build issue)
#[allow(clippy::expect_used)] // System misconfiguration or build issue; panics documented above
fn build_client(timeout: Duration, connect_timeout: Duration) -> Client {
    Client::builder()
        .user_agent("omg-package-manager")
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_nodelay(true)
        .build()
        .expect("Failed to build HTTP client - check TLS configuration")
}

/// Shared default HTTP client.
#[must_use]
pub fn shared_client() -> &'static Client {
    &SHARED_CLIENT
}

/// Shared HTTP client with extended timeouts for large downloads.
#[must_use]
pub fn download_client() -> &'static Client {
    &DOWNLOAD_CLIENT
}
