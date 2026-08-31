#![cfg(feature = "arch")]

//! Daemon cache hit/miss rates, invalidation, coherency, and memory pressure.

use anyhow::Result;
use omg_lib::daemon::cache::PackageCache;
use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::index::PackageIndex;
use omg_lib::daemon::protocol::{
    PackageInfo, Request, Response, ResponseResult, WirePackageSource,
};
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
        let package_manager = Arc::new(
            omg_lib::package_managers::mock::MockPackageManager::new_in("arch", &data_dir),
        );
        let state = Arc::new(DaemonState::new_isolated(
            &data_dir,
            PackageIndex::empty(),
            package_manager,
        )?);

        Ok(Self {
            _temp_dir: temp_dir,
            state,
        })
    }

    async fn send_request(&self, request: Request) -> Response {
        handle_request(Arc::clone(&self.state), request).await
    }

    async fn clear_cache(&self) {
        let response = self.send_request(Request::CacheClear { id: 0 }).await;
        assert!(
            matches!(response, Response::Success { .. }),
            "CacheClear request failed: {response:?}"
        );
    }
}

// ============================================================================
// Cache Hit/Miss Rates
// ============================================================================

#[tokio::test]
#[serial]
async fn test_cache_hit_rate_tracking() -> Result<()> {
    let fixture = CacheTestFixture::new()?;
    fixture.clear_cache().await;

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
// Cache Invalidation
// ============================================================================

#[tokio::test]
#[serial]
async fn test_explicit_cache_clear() -> Result<()> {
    let fixture = CacheTestFixture::new()?;

    async fn cache_misses(fixture: &CacheTestFixture, id: u64) -> u64 {
        match fixture.send_request(Request::Metrics { id }).await {
            Response::Success {
                result: ResponseResult::Metrics(m),
                ..
            } => m.cache_misses,
            response => panic!("Metrics request failed: {response:?}"),
        }
    }

    async fn search(fixture: &CacheTestFixture, id: u64) {
        fixture
            .send_request(Request::Search {
                id,
                query: "test".to_string(),
                limit: Some(10),
            })
            .await;
    }

    // Populate the cache and observe the miss.
    let misses_before = cache_misses(&fixture, 1).await;
    search(&fixture, 2).await;
    let misses_after_first = cache_misses(&fixture, 3).await;
    assert!(
        misses_after_first > misses_before,
        "first search must be a cache miss"
    );

    // Repeat: served from cache (no new miss).
    search(&fixture, 4).await;
    let misses_after_repeat = cache_misses(&fixture, 5).await;
    assert_eq!(
        misses_after_repeat, misses_after_first,
        "repeat search must be served from cache"
    );

    // Clear cache
    let clear_response = fixture.send_request(Request::CacheClear { id: 6 }).await;
    assert!(
        matches!(clear_response, Response::Success { .. }),
        "Cache clear should succeed"
    );

    // The same query must now be a miss again: invalidation is observable
    // through request semantics, not only through internal stats.
    search(&fixture, 7).await;
    let misses_after_clear = cache_misses(&fixture, 8).await;
    assert!(
        misses_after_clear > misses_after_repeat,
        "search after CacheClear must be a cache miss again"
    );

    Ok(())
}

// ============================================================================
// Repeated Status Consistency
// ============================================================================

#[tokio::test]
#[serial]
async fn test_repeated_status_reads_are_consistent() -> Result<()> {
    let fixture = CacheTestFixture::new()?;

    // First status request
    let status1 = fixture.send_request(Request::Status { id: 1 }).await;

    // A second live status read over unchanged isolated state must agree.
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
                "repeated total package counts must agree"
            );
            assert_eq!(
                s1.explicit_packages, s2.explicit_packages,
                "repeated explicit package counts must agree"
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
// Missing Package Errors
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
// Memory Pressure Handling
// ============================================================================

#[tokio::test]
#[serial]
async fn test_lru_eviction_behavior() -> Result<()> {
    // Create small cache (3 entries max)
    let cache = PackageCache::new(3, 300);

    // PackageCache budgets about 64 KiB per configured entry; 60 KiB payloads
    // make three entries fit while a fourth forces one LRU eviction.
    let result = |name: &str| {
        Arc::new(vec![PackageInfo {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: "x".repeat(60_000),
            source: WirePackageSource::Official,
        }])
    };

    // Three weighted entries fit within the configured byte budget.
    cache.insert_arc("query-1".to_string(), result("one"));
    cache.insert_arc("query-2".to_string(), result("two"));
    cache.insert_arc("query-3".to_string(), result("three"));
    cache.sync();

    // Access query-1 to mark it as recently used
    let _ = cache.get("query-1");
    cache.sync();

    // Insert query-4 (should evict LRU, which is query-2)
    cache.insert_arc("query-4".to_string(), result("four"));
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
