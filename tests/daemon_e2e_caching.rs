#![cfg(feature = "arch")]

//! S-tier E2E Tests: Daemon Caching Layer
//!
//! Comprehensive tests for cache hit/miss rates, invalidation, coherency,
//! memory pressure handling, and corruption recovery.

use anyhow::Result;
use omg_lib::daemon::cache::PackageCache;
use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::protocol::{Request, Response, ResponseResult};
use serial_test::serial;
use std::sync::Arc;
use tempfile::TempDir;

/// Test fixture for caching tests
struct CacheTestFixture {
    _temp_dir: TempDir,
    state: Arc<DaemonState>,
}

impl CacheTestFixture {
    fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&data_dir)?;

        #[expect(unsafe_code)]
        unsafe {
            std::env::set_var("OMG_DAEMON_DATA_DIR", &data_dir);
            std::env::set_var("OMG_DATA_DIR", &data_dir);
        }

        omg_lib::core::security::init_audit_logger()?;

        let state = Arc::new(DaemonState::new()?);

        Ok(Self {
            _temp_dir: temp_dir,
            state,
        })
    }

    async fn send_request(&self, request: Request) -> Response {
        handle_request(Arc::clone(&self.state), request).await
    }

    fn get_cache_stats(&self) -> (usize, usize) {
        let stats = self.state.cache.stats();
        (stats.size, stats.max_size)
    }

    fn clear_cache(&self) {
        self.state.cache.clear();
    }
}

// ============================================================================
// Test 1: Cache Hit/Miss Rates
// ============================================================================

#[tokio::test]
#[serial]
async fn test_cache_hit_rate_tracking() -> Result<()> {
    let fixture = CacheTestFixture::new()?;
    fixture.clear_cache();

    // Get initial metrics
    let metrics1 = fixture.send_request(Request::Metrics { id: 100 }).await;

    let (initial_hits, initial_misses) = match metrics1 {
        Response::Success {
            result: ResponseResult::Metrics(m),
            ..
        } => (m.cache_hits, m.cache_misses),
        response => panic!("Metrics request failed: {response:?}"),
    };

    // Perform a search (cache miss)
    fixture
        .send_request(Request::Search {
            id: 1,
            query: "test".to_string(),
            limit: Some(10),
        })
        .await;

    // Repeat same search (cache hit)
    fixture
        .send_request(Request::Search {
            id: 2,
            query: "test".to_string(),
            limit: Some(10),
        })
        .await;

    // Check metrics
    let metrics2 = fixture.send_request(Request::Metrics { id: 101 }).await;

    let metrics = match metrics2 {
        Response::Success {
            result: ResponseResult::Metrics(m),
            ..
        } => m,
        response => panic!("Metrics request failed: {response:?}"),
    };
    let hits_delta = metrics.cache_hits - initial_hits;
    let misses_delta = metrics.cache_misses - initial_misses;

    assert!(misses_delta >= 1, "Should have at least 1 cache miss");
    assert!(hits_delta >= 1, "Should have at least 1 cache hit");

    let hit_rate = hits_delta as f64 / (hits_delta + misses_delta) as f64;
    println!("Cache hit rate: {:.2}%", hit_rate * 100.0);

    Ok(())
}

// ============================================================================
// Test 2: Cache Invalidation
// ============================================================================

#[tokio::test]
#[serial]
async fn test_explicit_cache_clear() -> Result<()> {
    let fixture = CacheTestFixture::new()?;

    // Populate cache
    fixture
        .send_request(Request::Search {
            id: 1,
            query: "test".to_string(),
            limit: Some(10),
        })
        .await;

    // Sync cache operations (moka cache is eventually consistent)
    fixture.state.cache.sync();

    // Verify cache has entries
    let (size_before, _) = fixture.get_cache_stats();
    assert!(size_before > 0, "Cache should have entries");

    // Clear cache
    let clear_response = fixture.send_request(Request::CacheClear { id: 2 }).await;

    assert!(
        matches!(clear_response, Response::Success { .. }),
        "Cache clear should succeed"
    );

    // Sync cache operations
    fixture.state.cache.sync();

    // Verify cache is empty
    let (size_after, _) = fixture.get_cache_stats();
    assert_eq!(size_after, 0, "Cache should be empty after clear");

    Ok(())
}

// ============================================================================
// Test 3: Cache Coherency with System
// ============================================================================

#[tokio::test]
#[serial]
async fn test_status_cache_coherency() -> Result<()> {
    let fixture = CacheTestFixture::new()?;

    // First status request
    let status1 = fixture.send_request(Request::Status { id: 1 }).await;

    // Second status request (should be cached)
    let status2 = fixture.send_request(Request::Status { id: 2 }).await;

    // Both should succeed and return same data
    match (status1, status2) {
        (
            Response::Success {
                result: ResponseResult::Status(s1),
                ..
            },
            Response::Success {
                result: ResponseResult::Status(s2),
                ..
            },
        ) => {
            assert_eq!(
                s1.total_packages, s2.total_packages,
                "Cached status should match original"
            );
            assert_eq!(
                s1.explicit_packages, s2.explicit_packages,
                "Cached explicit count should match"
            );
        }
        _ => unreachable!("Status requests should succeed"),
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_package_info_cache_coherency() -> Result<()> {
    let fixture = CacheTestFixture::new()?;

    // First info request
    let info1 = fixture
        .send_request(Request::Info {
            id: 1,
            package: "bash".to_string(),
        })
        .await;

    // Second info request (should be cached)
    let info2 = fixture
        .send_request(Request::Info {
            id: 2,
            package: "bash".to_string(),
        })
        .await;

    // Both should return same data
    match (info1, info2) {
        (
            Response::Success {
                result: ResponseResult::Info(i1),
                ..
            },
            Response::Success {
                result: ResponseResult::Info(i2),
                ..
            },
        ) => {
            assert_eq!(i1.name, i2.name, "Cached info should match");
            assert_eq!(i1.version, i2.version, "Cached version should match");
        }
        (
            Response::Error {
                code: code1,
                message: message1,
                ..
            },
            Response::Error {
                code: code2,
                message: message2,
                ..
            },
        ) => {
            assert_eq!(code1, code2, "Cached errors should preserve the error code");
            assert_eq!(
                message1, message2,
                "Cached errors should preserve the message"
            );
        }
        _ => panic!("Repeated info requests returned different response variants"),
    }

    Ok(())
}

// ============================================================================
// Test 4: Missing Package Errors
// ============================================================================

#[tokio::test]
#[serial]
async fn test_missing_package_returns_error_consistently() -> Result<()> {
    let fixture = CacheTestFixture::new()?;
    let nonexistent_package = "this-package-definitely-does-not-exist-12345";

    let response1 = fixture
        .send_request(Request::Info {
            id: 1,
            package: nonexistent_package.to_string(),
        })
        .await;
    let response2 = fixture
        .send_request(Request::Info {
            id: 2,
            package: nonexistent_package.to_string(),
        })
        .await;

    match (response1, response2) {
        (
            Response::Error {
                code: code1,
                message: message1,
                ..
            },
            Response::Error {
                code: code2,
                message: message2,
                ..
            },
        ) => {
            assert_eq!(code1, code2, "Repeated missing-package errors should match");
            assert_eq!(
                message1, message2,
                "Repeated missing-package messages should match"
            );
        }
        _ => panic!("Missing package requests returned inconsistent responses"),
    }

    Ok(())
}

// ============================================================================
// Test 5: Memory Pressure Handling
// ============================================================================

#[tokio::test]
#[serial]
async fn test_lru_eviction_behavior() -> Result<()> {
    // Create small cache (3 entries max)
    let cache = PackageCache::new(3, 300);

    // Insert 3 entries
    cache.insert("query-1".to_string(), vec![]);
    cache.insert("query-2".to_string(), vec![]);
    cache.insert("query-3".to_string(), vec![]);
    cache.sync();

    // Access query-1 to mark it as recently used
    let _ = cache.get("query-1");
    cache.sync();

    // Insert query-4 (should evict LRU, which is query-2)
    cache.insert("query-4".to_string(), vec![]);
    cache.sync();

    // query-1 should still be cached (recently accessed), while query-2 is the LRU entry.
    assert!(
        cache.get("query-1").is_some(),
        "Recently accessed entry should not be evicted"
    );
    assert!(
        cache.get("query-2").is_none(),
        "Least-recently-used entry should be evicted"
    );

    Ok(())
}

// ============================================================================
// Test 6: Persistent Cache (Disk-backed)
// ============================================================================

// ============================================================================
// Test 8: Cache Performance Metrics
// ============================================================================
