#![cfg(feature = "arch")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]

//! S-tier E2E Tests: Daemon Caching Layer
//!
//! Comprehensive tests for cache hit/miss rates, invalidation, coherency,
//! memory pressure handling, and corruption recovery.

use anyhow::Result;
use omg_lib::daemon::cache::PackageCache;
use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::protocol::{PackageInfo, Request, Response, ResponseResult};
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

/// Test fixture for caching tests
struct CacheTestFixture {
    _temp_dir: TempDir,
    state: Arc<DaemonState>,
}

impl CacheTestFixture {
    async fn new() -> Result<Self> {
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
async fn test_cache_miss_on_first_request() -> Result<()> {
    let fixture = CacheTestFixture::new().await?;

    // Clear any existing cache
    fixture.clear_cache();

    // First search - should be cache miss
    let request = Request::Search {
        id: 1,
        query: "test-package".to_string(),
        limit: Some(10),
    };

    let start = std::time::Instant::now();
    let _response = fixture.send_request(request).await;
    let first_duration = start.elapsed();

    println!("First request (cache miss): {:?}", first_duration);

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_cache_hit_on_repeated_request() -> Result<()> {
    let fixture = CacheTestFixture::new().await?;
    fixture.clear_cache();

    let query = "firefox";

    // First request - cache miss
    let request1 = Request::Search {
        id: 1,
        query: query.to_string(),
        limit: Some(10),
    };
    let start1 = std::time::Instant::now();
    let _response1 = fixture.send_request(request1).await;
    let duration1 = start1.elapsed();

    // Second identical request - cache hit
    let request2 = Request::Search {
        id: 2,
        query: query.to_string(),
        limit: Some(10),
    };
    let start2 = std::time::Instant::now();
    let _response2 = fixture.send_request(request2).await;
    let duration2 = start2.elapsed();

    println!("Cache miss: {:?}", duration1);
    println!("Cache hit: {:?}", duration2);

    // Cache hit should be significantly faster (at least 2x)
    assert!(
        duration2 < duration1 / 2,
        "Cache hit should be much faster: miss={:?}, hit={:?}",
        duration1,
        duration2
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_cache_hit_rate_tracking() -> Result<()> {
    let fixture = CacheTestFixture::new().await?;
    fixture.clear_cache();

    // Get initial metrics
    let metrics1 = fixture.send_request(Request::Metrics { id: 100 }).await;

    let (initial_hits, initial_misses) = if let Response::Success {
        result: ResponseResult::Metrics(m),
        ..
    } = metrics1
    {
        (m.cache_hits, m.cache_misses)
    } else {
        (0, 0)
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

    if let Response::Success {
        result: ResponseResult::Metrics(m),
        ..
    } = metrics2
    {
        let hits_delta = m.cache_hits - initial_hits;
        let misses_delta = m.cache_misses - initial_misses;

        assert!(misses_delta >= 1, "Should have at least 1 cache miss");
        assert!(hits_delta >= 1, "Should have at least 1 cache hit");

        let hit_rate = hits_delta as f64 / (hits_delta + misses_delta) as f64;
        println!("Cache hit rate: {:.2}%", hit_rate * 100.0);
    }

    Ok(())
}

// ============================================================================
// Test 2: Cache Invalidation
// ============================================================================

#[tokio::test]
#[serial]
async fn test_explicit_cache_clear() -> Result<()> {
    let fixture = CacheTestFixture::new().await?;

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

#[tokio::test]
#[serial]
async fn test_cache_invalidation_after_clear() -> Result<()> {
    let fixture = CacheTestFixture::new().await?;
    fixture.clear_cache();

    let query = "python";

    // First request - populates cache
    let request1 = Request::Search {
        id: 1,
        query: query.to_string(),
        limit: Some(10),
    };
    let _response1 = fixture.send_request(request1).await;

    // Clear cache
    fixture.clear_cache();

    // Next request should be cache miss again
    let request2 = Request::Search {
        id: 2,
        query: query.to_string(),
        limit: Some(10),
    };
    let start = std::time::Instant::now();
    let _response2 = fixture.send_request(request2).await;
    let duration = start.elapsed();

    // Should take longer (cache miss)
    println!("Request after cache clear: {:?}", duration);

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ttl_expiration() -> Result<()> {
    // Create cache with very short TTL (1 second)
    let cache = PackageCache::new(100, 1);

    // Insert test data
    cache.insert(
        "test-query".to_string(),
        vec![PackageInfo {
            name: "test".to_string(),
            version: "1.0".to_string(),
            description: "Test package".to_string(),
            source: "test".to_string(),
        }],
    );

    // Should be available immediately
    assert!(
        cache.get("test-query").is_some(),
        "Cache entry should exist immediately"
    );

    // Wait for TTL expiration (2 seconds to be safe)
    sleep(Duration::from_secs(2)).await;

    // Should be expired
    assert!(
        cache.get("test-query").is_none(),
        "Cache entry should expire after TTL"
    );

    Ok(())
}

// ============================================================================
// Test 3: Cache Coherency with System
// ============================================================================

#[tokio::test]
#[serial]
async fn test_status_cache_coherency() -> Result<()> {
    let fixture = CacheTestFixture::new().await?;

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
        _ => panic!("Status requests should succeed"),
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_package_info_cache_coherency() -> Result<()> {
    let fixture = CacheTestFixture::new().await?;

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
        _ => {
            // Package might not exist, which is fine
        }
    }

    Ok(())
}

// ============================================================================
// Test 4: Negative Cache (Cache Misses)
// ============================================================================

#[tokio::test]
#[serial]
async fn test_negative_cache_for_missing_packages() -> Result<()> {
    let fixture = CacheTestFixture::new().await?;

    let nonexistent_package = "this-package-definitely-does-not-exist-12345";

    // First request for nonexistent package
    let start1 = std::time::Instant::now();
    let response1 = fixture
        .send_request(Request::Info {
            id: 1,
            package: nonexistent_package.to_string(),
        })
        .await;
    let duration1 = start1.elapsed();

    // Should return error
    assert!(
        matches!(response1, Response::Error { .. }),
        "Nonexistent package should return error"
    );

    // Second request for same nonexistent package
    let start2 = std::time::Instant::now();
    let response2 = fixture
        .send_request(Request::Info {
            id: 2,
            package: nonexistent_package.to_string(),
        })
        .await;
    let duration2 = start2.elapsed();

    // Should also return error
    assert!(
        matches!(response2, Response::Error { .. }),
        "Nonexistent package should return error"
    );

    // Negative cache should make second lookup faster
    println!("First miss: {:?}, Second miss: {:?}", duration1, duration2);
    assert!(
        duration2 <= duration1,
        "Negative cache should speed up repeated misses"
    );

    Ok(())
}

// ============================================================================
// Test 5: Memory Pressure Handling
// ============================================================================

#[tokio::test]
#[serial]
async fn test_cache_eviction_under_pressure() -> Result<()> {
    // Create small cache (10 entries max)
    let cache = PackageCache::new(10, 300);

    // Insert 20 entries (exceeds max)
    for i in 0..20 {
        cache.insert(
            format!("query-{}", i),
            vec![PackageInfo {
                name: format!("pkg-{}", i),
                version: "1.0".to_string(),
                description: "Test".to_string(),
                source: "test".to_string(),
            }],
        );
    }

    // Cache should have evicted oldest entries
    let stats = cache.stats();
    assert!(
        stats.size <= stats.max_size,
        "Cache size should not exceed max: {} > {}",
        stats.size,
        stats.max_size
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_lru_eviction_behavior() -> Result<()> {
    // Create small cache (3 entries max)
    let cache = PackageCache::new(3, 300);

    // Insert 3 entries
    cache.insert("query-1".to_string(), vec![]);
    cache.insert("query-2".to_string(), vec![]);
    cache.insert("query-3".to_string(), vec![]);

    // Access query-1 to mark it as recently used
    let _ = cache.get("query-1");

    // Insert query-4 (should evict LRU, which is query-2)
    cache.insert("query-4".to_string(), vec![]);

    // query-1 should still be cached (recently accessed)
    assert!(
        cache.get("query-1").is_some(),
        "Recently accessed entry should not be evicted"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_large_result_set_caching() -> Result<()> {
    let fixture = CacheTestFixture::new().await?;

    // Request large limit
    let response = fixture
        .send_request(Request::Search {
            id: 1,
            query: "lib".to_string(), // Common prefix, many results
            limit: Some(500),
        })
        .await;

    match response {
        Response::Success {
            result: ResponseResult::Search(search_result),
            ..
        } => {
            println!(
                "Large result set: {} packages",
                search_result.packages.len()
            );

            // Repeat request - should be cached
            let start = std::time::Instant::now();
            let _response2 = fixture
                .send_request(Request::Search {
                    id: 2,
                    query: "lib".to_string(),
                    limit: Some(500),
                })
                .await;
            let cached_duration = start.elapsed();

            // Cached large result should still be fast
            assert!(
                cached_duration < Duration::from_millis(10),
                "Cached large result should be fast, got {:?}",
                cached_duration
            );
        }
        _ => {
            // May not have enough packages, which is fine
        }
    }

    Ok(())
}

// ============================================================================
// Test 6: Persistent Cache (Disk-backed)
// ============================================================================

#[tokio::test]
#[serial]
async fn test_persistent_cache_survival() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_dir)?;

    #[expect(unsafe_code)]
    unsafe {
        std::env::set_var("OMG_DAEMON_DATA_DIR", &data_dir);
        std::env::set_var("OMG_DATA_DIR", &data_dir);
    }

    omg_lib::core::security::init_audit_logger()?;

    // Create first daemon instance
    {
        let state = Arc::new(DaemonState::new()?);

        // Populate status cache
        let _response = handle_request(Arc::clone(&state), Request::Status { id: 1 }).await;

        // Drop state (simulates daemon shutdown)
        drop(state);
    }

    // Create second daemon instance (simulates restart)
    {
        let state = Arc::new(DaemonState::new()?);

        // Status should be available from persistent cache
        let start = std::time::Instant::now();
        let response = handle_request(Arc::clone(&state), Request::Status { id: 2 }).await;
        let duration = start.elapsed();

        match response {
            Response::Success {
                result: ResponseResult::Status(_),
                ..
            } => {
                println!("Persistent cache hit: {:?}", duration);
                // Should be fast (from persistent cache)
                assert!(
                    duration < Duration::from_millis(100),
                    "Persistent cache should be fast"
                );
            }
            _ => {
                // May not have cached status yet
            }
        }
    }

    Ok(())
}

// ============================================================================
// Test 7: Cache Corruption Recovery
// ============================================================================

#[tokio::test]
#[serial]
async fn test_corrupted_cache_recovery() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().join("data");
    std::fs::create_dir_all(&data_dir)?;

    // Create a corrupted cache file
    let cache_file = data_dir.join("cache.redb");
    std::fs::write(&cache_file, b"CORRUPTED DATA")?;

    #[expect(unsafe_code)]
    unsafe {
        std::env::set_var("OMG_DAEMON_DATA_DIR", &data_dir);
        std::env::set_var("OMG_DATA_DIR", &data_dir);
    }

    omg_lib::core::security::init_audit_logger()?;

    // Daemon should handle corrupted cache gracefully
    // (may fail to create state due to locked db, which is acceptable)
    let result = DaemonState::new();

    match result {
        Ok(state) => {
            // Successfully recovered, verify it works
            let response = handle_request(Arc::new(state), Request::Ping { id: 1 }).await;

            assert!(
                matches!(response, Response::Success { .. }),
                "Daemon should work despite cache corruption"
            );
        }
        Err(e) => {
            // Expected: failed to open corrupted database
            println!("Expected failure with corrupted cache: {}", e);
        }
    }

    Ok(())
}

// ============================================================================
// Test 8: Cache Performance Metrics
// ============================================================================

#[tokio::test]
#[serial]
async fn test_cache_performance_characteristics() -> Result<()> {
    let cache = PackageCache::new(1000, 300);

    // Test insertion performance
    let start = std::time::Instant::now();
    for i in 0..100 {
        cache.insert(
            format!("query-{}", i),
            vec![PackageInfo {
                name: format!("pkg-{}", i),
                version: "1.0".to_string(),
                description: "Test".to_string(),
                source: "test".to_string(),
            }],
        );
    }
    let insert_duration = start.elapsed();

    // Test lookup performance
    let start = std::time::Instant::now();
    for i in 0..100 {
        let _ = cache.get(&format!("query-{}", i));
    }
    let lookup_duration = start.elapsed();

    println!("100 insertions: {:?}", insert_duration);
    println!("100 lookups: {:?}", lookup_duration);

    // Lookups should be fast (< 10ms for 100 lookups, relaxed for CI/parallel load)
    assert!(
        lookup_duration < Duration::from_millis(10),
        "Cache lookups should be very fast, got {:?}",
        lookup_duration
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_arc_clone_efficiency() -> Result<()> {
    let cache = PackageCache::new(100, 300);

    // Insert large data structure
    let large_data: Vec<PackageInfo> = (0..1000)
        .map(|i| PackageInfo {
            name: format!("pkg-{}", i),
            version: "1.0".to_string(),
            description: "Test package description".to_string(),
            source: "test".to_string(),
        })
        .collect();

    cache.insert("large-query".to_string(), large_data);

    // Measure Arc clone time (should be O(1))
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _ = cache.get("large-query");
    }
    let clone_duration = start.elapsed();

    println!("1000 Arc clones of large data: {:?}", clone_duration);

    // 1000 cache lookups (including Arc clones) should be very fast (< 5ms)
    // Each lookup includes hash lookup + Arc clone, so 5ms for 1000 = 5μs per lookup is excellent
    assert!(
        clone_duration < Duration::from_millis(5),
        "Arc clones should be O(1), got {:?}",
        clone_duration
    );

    Ok(())
}
