//! Shared HTTP client utilities
//!
//! Centralizes reqwest client configuration for connection pooling
//! and consistent timeouts across the codebase.

use std::sync::LazyLock;
use std::time::Duration;

use reqwest::{Client, Url};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(5);
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(60);

static SHARED_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    build_client(
        DEFAULT_TIMEOUT,
        DEFAULT_CONNECT_TIMEOUT,
        DEFAULT_READ_TIMEOUT,
    )
});

/// Download client with extended timeouts and read timeout for stall detection.
///
/// Uses `.read_timeout()` to detect stalled downloads - this timeout resets after
/// each successful read, unlike `.timeout()` which covers the entire request.
#[expect(clippy::expect_used)] // System misconfiguration or build issue; documented in build_client
static DOWNLOAD_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent("omg-package-manager")
        .timeout(DOWNLOAD_TIMEOUT)
        .connect_timeout(DOWNLOAD_CONNECT_TIMEOUT)
        .read_timeout(DOWNLOAD_READ_TIMEOUT)
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_nodelay(true)
        .build()
        .expect("Failed to build download HTTP client - check TLS configuration")
});

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
#[expect(clippy::expect_used)] // System misconfiguration or build issue; panics documented above
fn build_client(timeout: Duration, connect_timeout: Duration, read_timeout: Duration) -> Client {
    Client::builder()
        .user_agent("omg-package-manager")
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        .read_timeout(read_timeout)
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_nodelay(true)
        .build()
        .expect("Failed to build HTTP client - check TLS configuration")
}

/// Render a remote URL without credentials, query parameters, or fragments.
///
/// Error messages and logs must not echo URL-embedded tokens. Invalid URLs are
/// represented by a fixed placeholder rather than reflecting unparsed input.
#[must_use]
pub fn redact_url(raw: &str) -> String {
    let Ok(mut url) = Url::parse(raw) else {
        return "<invalid URL>".to_string();
    };
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

/// Shared default HTTP client.
#[must_use]
#[inline]
pub fn shared_client() -> &'static Client {
    &SHARED_CLIENT
}

/// Shared HTTP client with extended timeouts for large downloads.
#[must_use]
#[inline]
pub fn download_client() -> &'static Client {
    &DOWNLOAD_CLIENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacted_urls_never_reflect_credentials_or_query_secrets() {
        let redacted = redact_url("https://user:password@example.com/file?token=secret#fragment");
        assert_eq!(redacted, "https://example.com/file");
        for secret in ["user", "password", "token", "secret", "fragment"] {
            assert!(!redacted.contains(secret));
        }
        assert_eq!(redact_url("not a URL with secret"), "<invalid URL>");
    }
}
