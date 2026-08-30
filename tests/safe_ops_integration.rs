//! Integration tests for safe operations module
//!
//! This module tests the safe operations in more realistic scenarios
//! to ensure they work correctly with the broader codebase.

use omg_lib::core::safe_ops::*;
use tempfile::TempDir;
use tokio::fs;

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
    let result = validate_path_syntax(valid_path);
    assert!(result.is_ok());

    // Test empty path
    let empty_result = validate_path_syntax("");
    assert!(empty_result.is_err());

    // Test path with null byte
    let null_path = "/tmp/with\0null";
    let null_result = validate_path_syntax(null_path);
    assert!(null_result.is_err());
}

#[test]
fn test_nonzero_fallback_edge_cases() {
    // Test with default fallback
    let nz_default = nonzero_u32_or_default(0, 999);
    assert_eq!(nz_default.get(), 999);

    let nz_valid = nonzero_u32_or_default(123, 999);
    assert_eq!(nz_valid.get(), 123);
}
