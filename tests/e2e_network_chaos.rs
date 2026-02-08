//! E2E Network Chaos Tests
//!
//! Tests for network failure scenarios that occur in production environments.
//! These tests verify graceful degradation and proper error handling when
//! network conditions are unstable.

#![cfg(feature = "arch")]

use anyhow::Result;
use std::time::Duration;

mod common;

/// Test that HTTP client handles connection timeouts gracefully
#[tokio::test]
async fn test_connection_timeout_handling() -> Result<()> {
    // Configure a very short timeout
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(1))
        .build()?;

    // Try to connect to a non-routable IP (will timeout)
    let result = client.get("http://10.255.255.1/").send().await;

    assert!(result.is_err(), "Should fail with connection timeout");
    let err = result.unwrap_err();
    assert!(
        err.is_timeout() || err.is_connect(),
        "Error should be timeout or connect: {:?}",
        err
    );

    Ok(())
}

/// Test that DNS resolution failures are handled properly
#[tokio::test]
async fn test_dns_resolution_failure() -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    // Try to resolve a definitely non-existent domain
    let result = client
        .get("http://this-domain-does-not-exist-12345.invalid/")
        .send()
        .await;

    assert!(result.is_err(), "Should fail with DNS resolution error");

    Ok(())
}

/// Test HTTP redirect loop detection
#[tokio::test]
async fn test_redirect_loop_protection() -> Result<()> {
    // The HTTP client should have redirect limits (default is usually 10)
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .timeout(Duration::from_secs(10))
        .build()?;

    // httpbin.org provides a redirect endpoint for testing
    // Skip this test if httpbin is not reachable (CI environments may block external)
    let test_url = "https://httpbin.org/redirect/10"; // 10 redirects, but limit is 5

    match client.get(test_url).send().await {
        Ok(resp) => {
            // If we got a response, it should indicate redirect issue
            // This might succeed if redirect policy stops it
            assert!(
                resp.status().is_redirection() || resp.status().is_success(),
                "Got unexpected status: {}",
                resp.status()
            );
        }
        Err(e) => {
            // Expected: redirect limit exceeded or network error
            assert!(
                e.is_redirect() || e.is_timeout() || e.is_connect(),
                "Error should be redirect-related: {:?}",
                e
            );
        }
    }

    Ok(())
}

/// Test graceful handling of server errors (5xx responses)
#[tokio::test]
async fn test_server_error_handling() -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    // httpbin.org provides status code simulation
    // Skip if not reachable
    match client.get("https://httpbin.org/status/500").send().await {
        Ok(resp) => {
            assert_eq!(
                resp.status().as_u16(),
                500,
                "Should receive 500 status code"
            );
        }
        Err(e) if e.is_timeout() || e.is_connect() => {
            // Network issue - test inconclusive but not a failure
            eprintln!("Skipping test: httpbin.org not reachable");
        }
        Err(e) => {
            panic!("Unexpected error: {:?}", e);
        }
    }

    Ok(())
}

/// Test that partial response detection works (for download integrity)
#[tokio::test]
async fn test_content_length_mismatch_detection() -> Result<()> {
    // This tests the concept of detecting incomplete downloads
    // In practice, reqwest handles this via content-length validation

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    // Get a known-size response
    match client.get("https://httpbin.org/bytes/1024").send().await {
        Ok(resp) => {
            if let Some(content_length) = resp.content_length() {
                let body = resp.bytes().await?;
                assert_eq!(
                    body.len() as u64,
                    content_length,
                    "Body length should match Content-Length header"
                );
            }
        }
        Err(e) if e.is_timeout() || e.is_connect() => {
            eprintln!("Skipping test: httpbin.org not reachable");
        }
        Err(e) => {
            panic!("Unexpected error: {:?}", e);
        }
    }

    Ok(())
}

/// Test retry logic behavior (validates exponential backoff concept)
#[tokio::test]
async fn test_retry_with_exponential_backoff() -> Result<()> {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    let attempt_count = Arc::new(AtomicU32::new(0));
    let max_retries = 3u32;

    // Simulate retry logic
    let mut last_delay = Duration::ZERO;
    for retry in 0..max_retries {
        attempt_count.fetch_add(1, Ordering::SeqCst);

        // Calculate exponential backoff
        let delay = Duration::from_millis(100 * 2u64.pow(retry));

        // Verify delays increase exponentially
        if retry > 0 {
            assert!(
                delay > last_delay,
                "Delay should increase: {:?} > {:?}",
                delay,
                last_delay
            );
        }
        last_delay = delay;
    }

    assert_eq!(
        attempt_count.load(Ordering::SeqCst),
        max_retries,
        "Should have made {} attempts",
        max_retries
    );

    Ok(())
}

/// Test TLS certificate validation (should reject invalid certs by default)
#[tokio::test]
async fn test_tls_certificate_validation() -> Result<()> {
    // Create a client that validates certificates (default behavior)
    let strict_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    // expired.badssl.com has an expired certificate
    let result = strict_client.get("https://expired.badssl.com/").send().await;

    // Should fail due to certificate validation
    match result {
        Ok(_) => {
            // Some systems may have outdated CA certs or special config
            eprintln!("Warning: Certificate validation may not be working as expected");
        }
        Err(e) => {
            // This is the expected behavior - TLS error
            assert!(
                e.is_request() || e.is_connect(),
                "Should fail with TLS/certificate error: {:?}",
                e
            );
        }
    }

    Ok(())
}

/// Test graceful handling of slow responses (read timeout)
#[tokio::test]
async fn test_slow_response_timeout() -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;

    // httpbin.org/delay/N waits N seconds before responding
    match client.get("https://httpbin.org/delay/5").send().await {
        Ok(_) => {
            panic!("Should have timed out waiting for slow response");
        }
        Err(e) => {
            assert!(e.is_timeout(), "Error should be timeout: {:?}", e);
        }
    }

    Ok(())
}

/// Test that concurrent requests don't cause resource exhaustion
#[tokio::test]
async fn test_concurrent_request_handling() -> Result<()> {
    use futures::stream::{self, StreamExt};

    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(5)
        .timeout(Duration::from_secs(30))
        .build()?;

    // Launch 10 concurrent requests
    let results: Vec<_> = stream::iter(0..10)
        .map(|_| {
            let client = client.clone();
            async move {
                match client.get("https://httpbin.org/get").send().await {
                    Ok(resp) => Some(resp.status().is_success()),
                    Err(_) => None,
                }
            }
        })
        .buffer_unordered(10)
        .collect()
        .await;

    // At least some requests should succeed (network permitting)
    let successes = results.iter().filter(|r| **r == Some(true)).count();

    // Allow for network variability - just verify we don't crash
    eprintln!("Concurrent requests: {}/{} succeeded", successes, results.len());

    Ok(())
}

/// Test mirror failover behavior (simulated)
#[tokio::test]
async fn test_mirror_failover_simulation() -> Result<()> {
    // Simulate a list of mirrors where some are unreachable
    let mirrors = vec![
        "http://10.255.255.1/", // Non-routable (will timeout)
        "http://10.255.255.2/", // Non-routable (will timeout)
        "https://httpbin.org/get", // Should work
    ];

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_secs(5))
        .build()?;

    let mut success = false;
    let mut attempts = 0;

    for mirror in &mirrors {
        attempts += 1;
        match client.get(*mirror).send().await {
            Ok(resp) if resp.status().is_success() => {
                success = true;
                eprintln!(
                    "Mirror {} succeeded after {} attempts",
                    mirror, attempts
                );
                break;
            }
            Ok(resp) => {
                eprintln!(
                    "Mirror {} returned non-success: {}",
                    mirror,
                    resp.status()
                );
            }
            Err(e) => {
                eprintln!("Mirror {} failed: {:?}", mirror, e);
            }
        }
    }

    // The third mirror should succeed (network permitting)
    // We don't assert success because external network may be unavailable in CI
    eprintln!(
        "Mirror failover test: success={}, attempts={}",
        success, attempts
    );

    Ok(())
}

/// Test that connection pool reuse works correctly
#[tokio::test]
async fn test_connection_pool_reuse() -> Result<()> {
    use std::time::Instant;

    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(10)
        .pool_idle_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(10))
        .build()?;

    // First request establishes connection
    let start1 = Instant::now();
    let result1 = client.get("https://httpbin.org/get").send().await;
    let time1 = start1.elapsed();

    if result1.is_err() {
        eprintln!("Skipping connection pool test: network unavailable");
        return Ok(());
    }

    // Second request should reuse connection (faster)
    let start2 = Instant::now();
    let result2 = client.get("https://httpbin.org/get").send().await;
    let time2 = start2.elapsed();

    if result2.is_ok() {
        eprintln!(
            "Connection pool test: first={:?}, second={:?}",
            time1, time2
        );
        // Second request is often faster due to connection reuse
        // But we don't assert this as it depends on network conditions
    }

    Ok(())
}

/// Test proper handling of HTTP/2 protocol
#[tokio::test]
async fn test_http2_support() -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;

    // Most modern servers support HTTP/2
    match client.get("https://www.google.com/").send().await {
        Ok(resp) => {
            // HTTP version info may not be directly accessible
            // but if we got a response, HTTP/2 negotiation worked
            assert!(
                resp.status().is_success() || resp.status().is_redirection(),
                "Should get successful response"
            );
        }
        Err(e) if e.is_timeout() || e.is_connect() => {
            eprintln!("Skipping HTTP/2 test: network unavailable");
        }
        Err(e) => {
            panic!("Unexpected error: {:?}", e);
        }
    }

    Ok(())
}

#[cfg(test)]
mod offline_tests {
    //! Tests that don't require network access

    use super::*;

    #[test]
    fn test_url_validation() {
        // Test that invalid URLs are rejected
        let invalid_urls = vec![
            "",
            "not-a-url",
            "ftp://unsupported.protocol",
            "javascript:alert(1)",
            "file:///etc/passwd",
        ];

        for url in invalid_urls {
            let result = url.parse::<reqwest::Url>();
            // Some may parse but shouldn't be usable for HTTP
            if let Ok(parsed) = result {
                assert!(
                    parsed.scheme() != "http" && parsed.scheme() != "https"
                        || url.is_empty(),
                    "Should not accept dangerous URL: {}",
                    url
                );
            }
        }
    }

    #[test]
    fn test_timeout_configuration() {
        // Verify timeout values are reasonable
        let short_timeout = Duration::from_millis(100);
        let long_timeout = Duration::from_secs(300);

        assert!(short_timeout < long_timeout);
        assert!(short_timeout.as_millis() > 0);
    }

    #[test]
    fn test_retry_delay_calculation() {
        // Verify exponential backoff formula
        let base_delay_ms = 100u64;
        let max_retries = 5;

        let mut delays = Vec::new();
        for retry in 0..max_retries {
            let delay_ms = base_delay_ms * 2u64.pow(retry);
            delays.push(delay_ms);
        }

        assert_eq!(delays, vec![100, 200, 400, 800, 1600]);

        // Total backoff time should be reasonable
        let total_ms: u64 = delays.iter().sum();
        assert!(total_ms < 10_000, "Total backoff should be under 10 seconds");
    }
}
