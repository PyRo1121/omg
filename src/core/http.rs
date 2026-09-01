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
const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(60);

static SHARED_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    build_client(
        Some(DEFAULT_TIMEOUT),
        DEFAULT_CONNECT_TIMEOUT,
        DEFAULT_READ_TIMEOUT,
    )
});

/// Download client with extended timeouts and read timeout for stall detection.
///
/// Uses `.read_timeout()` to detect stalled downloads - this timeout resets after
/// each successful read, unlike `.timeout()` which covers the entire request.
static DOWNLOAD_CLIENT: LazyLock<Client> =
    LazyLock::new(|| build_client(None, DOWNLOAD_CONNECT_TIMEOUT, DOWNLOAD_READ_TIMEOUT));

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
fn build_client(
    timeout: Option<Duration>,
    connect_timeout: Duration,
    read_timeout: Duration,
) -> Client {
    let mut builder = Client::builder()
        .user_agent("omg-package-manager")
        .connect_timeout(connect_timeout)
        .read_timeout(read_timeout)
        .pool_max_idle_per_host(32)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_nodelay(true);
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    builder
        .build()
        .expect("Failed to build HTTP client - check TLS configuration")
}

/// Calculate bounded exponential backoff for a zero-based retry number.
///
/// The exponent is capped so attacker-influenced retry counters cannot overflow
/// or create effectively unbounded sleeps.
#[must_use]
pub fn retry_backoff(initial: Duration, retry_number: u32) -> Duration {
    initial.saturating_mul(1_u32 << retry_number.min(20))
}

/// Whether an HTTP status represents a transient server-side failure.
#[must_use]
pub fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status.is_server_error()
}

/// Whether a request transport failure is safe to retry.
#[must_use]
pub fn is_retryable_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
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

    #[tokio::test]
    async fn download_client_allows_long_transfers_that_keep_progressing() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request).await.unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n")
                .await
                .unwrap();
            for byte in b"progress" {
                stream.write_all(&[*byte]).await.unwrap();
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });

        // Read timeout must stay well above the 50ms inter-byte gap so CI
        // scheduling jitter cannot look like a stalled download.
        let client = build_client(None, Duration::from_secs(1), Duration::from_secs(5));
        let body = client
            .get(format!("http://{address}"))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(&body[..], b"progress");
        server.await.unwrap();
    }

    #[test]
    fn retry_policy_is_bounded_and_rejects_client_errors() {
        assert_eq!(
            retry_backoff(Duration::from_millis(100), 0),
            Duration::from_millis(100)
        );
        assert_eq!(
            retry_backoff(Duration::from_millis(100), 2),
            Duration::from_millis(400)
        );
        assert_eq!(
            retry_backoff(Duration::MAX, u32::MAX),
            Duration::MAX,
            "backoff arithmetic must saturate"
        );
        assert!(is_retryable_status(reqwest::StatusCode::BAD_GATEWAY));
        assert!(!is_retryable_status(reqwest::StatusCode::NOT_FOUND));
    }

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
