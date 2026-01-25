#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! Integration tests for safe operations module
//!
//! This module tests the safe operations in more realistic scenarios
//! to ensure they work correctly with the broader codebase.

use omg_lib::core::safe_ops::*;
use tempfile::TempDir;
use tokio::fs;

#[tokio::test]
async fn test_safe_rate_limiter_integration() {
    // Test that safe rate limiter config works with governor
    let config = RateLimiterConfig::new().unwrap();
    assert_eq!(config.requests_per_second.get(), 100);
    assert_eq!(config.burst_size.get(), 200);

    // Test custom values
    let custom_config = RateLimiterConfig::with_values(50, 150).unwrap();
    assert_eq!(custom_config.requests_per_second.get(), 50);
    assert_eq!(custom_config.burst_size.get(), 150);
}

#[tokio::test]
async fn test_safe_file_operations_integration() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("integration_test.txt");
    let content = b"Integration test content";

    // Test async atomic write
    let result = atomic_write_file(&file_path, content).await;
    assert!(result.is_ok());

    // Verify content was written correctly
    let read_content = fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(read_content, "Integration test content");

    // Test sync atomic write
    let sync_path = temp_dir.path().join("sync_test.txt");
    let sync_result = atomic_write_file_sync(&sync_path, content);
    assert!(sync_result.is_ok());

    // Verify sync content
    let sync_read = std::fs::read_to_string(&sync_path).unwrap();
    assert_eq!(sync_read, "Integration test content");
}

#[tokio::test]
async fn test_path_validation_integration() {
    // Test valid path
    let temp_dir = TempDir::new().unwrap();
    let valid_path = temp_dir.path();
    let result = validate_path(valid_path);
    assert!(result.is_ok());

    // Test empty path
    let empty_result = validate_path("");
    assert!(empty_result.is_err());

    // Test path with null byte
    let null_path = "/tmp/with\0null";
    let null_result = validate_path(null_path);
    assert!(null_result.is_err());
}

#[test]
fn test_transaction_guard_integration() {
    // Test that transaction guard works correctly
    let transaction_data = "important_transaction_data";
    let guard = TransactionGuard::new(transaction_data);

    // Test accessing data
    assert_eq!(guard.inner(), &"important_transaction_data");

    // Test mutable access
    let mut mutable_guard = TransactionGuard::new("mutable_data");
    *mutable_guard.inner_mut() = "modified_data";
    assert_eq!(mutable_guard.inner(), &"modified_data");

    // Test commit
    let committed_data = TransactionGuard::new("commit_test").commit();
    assert_eq!(committed_data, "commit_test");
}

#[test]
fn test_atomic_counter_integration() {
    let counter = AtomicCounter::new(10);

    // Test initial value
    assert_eq!(counter.get(), 10);

    // Test increment
    let new_value = counter.increment();
    assert_eq!(new_value, 11);
    assert_eq!(counter.get(), 11);

    // Test reset
    counter.reset(5);
    assert_eq!(counter.get(), 5);
}

#[test]
fn test_expect_or_error_handling() {
    // Test Some value
    let some_value = Some(42);
    let result = expect_or(some_value, "test context");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 42);

    // Test None value
    let none_value: Option<i32> = None;
    let error_result = expect_or(none_value, "test context");
    assert!(error_result.is_err());
    assert!(
        error_result
            .unwrap_err()
            .to_string()
            .contains("Expected value for test context")
    );
}

#[test]
fn test_nonzero_constructors_edge_cases() {
    // Test boundary value of 1 (smallest valid)
    let nz1 = nonzero_u32(1, "boundary test").unwrap();
    assert_eq!(nz1.get(), 1);

    // Test large values
    let nz_large = nonzero_u64(u64::MAX, "max test").unwrap();
    assert_eq!(nz_large.get(), u64::MAX);

    // Test with default fallback
    let nz_default = nonzero_u32_or_default(0, 999);
    assert_eq!(nz_default.get(), 999);

    let nz_valid = nonzero_u32_or_default(123, 999);
    assert_eq!(nz_valid.get(), 123);
}
