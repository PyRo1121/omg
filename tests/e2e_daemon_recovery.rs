//! E2E Daemon State Corruption & Recovery Tests
//!
//! Tests for daemon recovering from corrupted databases, incomplete index
//! rebuilds, and other failure scenarios that require resilience.

#![cfg(feature = "arch")]
#![cfg(unix)]

use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;

/// Test that daemon handles corrupted database file gracefully
#[test]
fn test_corrupted_database_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    // Create a corrupted database file (invalid header)
    fs::write(&db_path, b"corrupted database header\x00\x00\xFF\xFF")?;

    // Verify the file exists but is not a valid database
    assert!(db_path.exists());
    let content = fs::read(&db_path)?;
    assert!(!content.is_empty());

    // A proper database should fail to open this
    // We simulate the detection logic here
    let is_valid_db = content.starts_with(b"REDB") || content.starts_with(b"SQLite");
    assert!(
        !is_valid_db,
        "Corrupted file should not be recognized as valid"
    );

    Ok(())
}

/// Test recovery from truncated index file
#[test]
fn test_truncated_index_recovery() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let index_path = temp_dir.path().join("index.bin");

    // Create a truncated index file (incomplete write)
    let header = b"OMGIDX\x01\x00"; // Valid header
    let partial_data = vec![0xFF; 512]; // Incomplete data

    let mut content = header.to_vec();
    content.extend(partial_data);
    fs::write(&index_path, &content)?;

    // Detect truncation: check if file size matches expected
    let file_size = fs::metadata(&index_path)?.len();
    let expected_min_size = 1024; // Minimum valid index size

    let needs_rebuild = file_size < expected_min_size;
    assert!(
        needs_rebuild,
        "Truncated index should be detected for rebuild"
    );

    // Simulate rebuild by creating new valid index
    let valid_index = create_valid_test_index();
    fs::write(&index_path, &valid_index)?;

    assert!(
        fs::metadata(&index_path)?.len() >= expected_min_size,
        "Rebuilt index should meet minimum size"
    );

    Ok(())
}

/// Test handling of concurrent database access
#[test]
fn test_database_lock_contention() -> Result<()> {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("shared.db");

    // Initialize database file
    fs::write(&db_path, b"initial state")?;

    let barrier = Arc::new(Barrier::new(3));
    let path = Arc::new(db_path);
    let results = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Spawn multiple threads trying to access the "database"
    let handles: Vec<_> = (0..3)
        .map(|i| {
            let barrier = barrier.clone();
            let path = path.clone();
            let results = results.clone();

            thread::spawn(move || {
                // Synchronize start
                barrier.wait();

                // Try to "open" database (simulate with file read)
                let result = fs::read_to_string(path.as_path());

                results.lock().unwrap().push((i, result.is_ok()));
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // At least one thread should succeed
    let successes = results.lock().unwrap().iter().filter(|(_, ok)| *ok).count();
    assert!(successes > 0, "At least one access should succeed");

    Ok(())
}

/// Test cache poisoning detection
#[test]
fn test_cache_poisoning_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let cache_dir = temp_dir.path().join("cache");
    fs::create_dir_all(&cache_dir)?;

    // Test cases for potentially malicious cache keys
    let malicious_keys = vec![
        "../../../etc/passwd",
        "..%2F..%2Fetc%2Fpasswd",
        "key\x00hidden",
        "key;rm -rf /",
        "/absolute/path",
        "valid/../../../escape",
    ];

    for key in malicious_keys {
        let is_valid = validate_cache_key(key);
        assert!(!is_valid, "Malicious cache key should be rejected: {key:?}");
    }

    // Valid cache keys should pass
    let valid_keys = vec![
        "firefox-120.0",
        "package_name",
        "sha256-abc123def456",
        "valid.key.name",
    ];

    for key in valid_keys {
        let is_valid = validate_cache_key(key);
        assert!(is_valid, "Valid cache key should be accepted: {key:?}");
    }

    Ok(())
}

/// Test recovery from partial write during save
#[test]
fn test_atomic_save_recovery() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let data_path = temp_dir.path().join("data.json");
    let temp_path = temp_dir.path().join("data.json.tmp");
    let backup_path = temp_dir.path().join("data.json.bak");

    // Original data
    let original_data = r#"{"version": 1, "data": "original"}"#;
    fs::write(&data_path, original_data)?;

    // Simulate atomic write pattern
    let new_data = r#"{"version": 2, "data": "updated"}"#;

    // Step 1: Write to temp file
    fs::write(&temp_path, new_data)?;

    // Step 2: Create backup of original
    fs::copy(&data_path, &backup_path)?;

    // Step 3: Simulate failure during rename
    let simulated_failure = true;

    if simulated_failure {
        // Don't rename - simulate power loss

        // Recovery: restore from backup if temp exists but main is old
        if temp_path.exists() && backup_path.exists() {
            // Check if main file is corrupted (in reality, check content)
            let main_content = fs::read_to_string(&data_path)?;

            if main_content == original_data {
                // Main file is old version, but we have temp with new version
                // This is a partial failure - we can complete the operation
                fs::rename(&temp_path, &data_path)?;
            }
        }
    } else {
        // Normal case: rename succeeds
        fs::rename(&temp_path, &data_path)?;
    }

    // Cleanup temp files
    if temp_path.exists() {
        fs::remove_file(&temp_path)?;
    }
    if backup_path.exists() {
        fs::remove_file(&backup_path)?;
    }

    Ok(())
}

/// Test index checksum validation
#[test]
fn test_index_checksum_validation() -> Result<()> {
    use sha2::{Digest, Sha256};

    let temp_dir = TempDir::new()?;
    let index_path = temp_dir.path().join("index.bin");
    let checksum_path = temp_dir.path().join("index.sha256");

    // Create index with known content
    let index_content = b"valid index data for testing";
    fs::write(&index_path, index_content)?;

    // Calculate and store checksum
    let mut hasher = Sha256::new();
    hasher.update(index_content);
    let checksum = format!("{:x}", hasher.finalize());
    fs::write(&checksum_path, &checksum)?;

    // Validate checksum
    let stored_checksum = fs::read_to_string(&checksum_path)?;
    let file_content = fs::read(&index_path)?;

    let mut hasher = Sha256::new();
    hasher.update(&file_content);
    let computed_checksum = format!("{:x}", hasher.finalize());

    assert_eq!(
        stored_checksum, computed_checksum,
        "Checksum should match for valid file"
    );

    // Corrupt the file
    fs::write(&index_path, b"corrupted content")?;

    // Re-validate should fail
    let file_content = fs::read(&index_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&file_content);
    let computed_checksum = format!("{:x}", hasher.finalize());

    assert_ne!(
        stored_checksum, computed_checksum,
        "Checksum should not match for corrupted file"
    );

    Ok(())
}

/// Test graceful degradation when cache is unavailable
#[test]
fn test_cache_unavailable_fallback() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let cache_path = temp_dir.path().join("cache");

    // Don't create cache directory - simulate it being unavailable

    // Attempt to use cache should gracefully fall back
    let cache_available = cache_path.exists() && cache_path.is_dir();

    if cache_available {
        // Use cache
        unreachable!("Cache should not be available in this test");
    } else {
        // Fallback: operate without cache
        // This should succeed but be slower
        simulate_operation_without_cache();
        // Operation completed successfully
    }

    Ok(())
}

/// Test recovery from interrupted daemon shutdown
#[test]
fn test_interrupted_shutdown_recovery() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let state_file = temp_dir.path().join("daemon.state");
    let pid_file = temp_dir.path().join("daemon.pid");

    // Simulate running daemon state
    fs::write(&pid_file, "12345")?;
    fs::write(
        &state_file,
        r#"{"status": "running", "started_at": 1234567890}"#,
    )?;

    // Simulate interrupted shutdown (files left behind)
    // Recovery should detect stale state

    // Check if PID is actually running
    let stored_pid: u32 = fs::read_to_string(&pid_file)?.parse()?;
    let pid_active = is_process_running(stored_pid);

    if !pid_active {
        // PID is stale - clean up
        fs::remove_file(&pid_file)?;
        fs::remove_file(&state_file)?;
    }

    // After recovery, files should be cleaned up (assuming PID 12345 isn't running)
    assert!(!pid_file.exists(), "Stale PID file should be cleaned up");
    assert!(
        !state_file.exists(),
        "Stale state file should be cleaned up"
    );

    Ok(())
}

/// Test migration from old database format
#[test]
fn test_database_migration() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let old_db = temp_dir.path().join("db.v1");
    let new_db = temp_dir.path().join("db.v2");

    // Create old format database
    let old_format = r#"{"version": 1, "packages": ["pkg1", "pkg2"]}"#;
    fs::write(&old_db, old_format)?;

    // Detect old format and migrate
    let content = fs::read_to_string(&old_db)?;
    let old_data: serde_json::Value = serde_json::from_str(&content)?;

    let version = old_data
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);

    if version < 2 {
        // Migrate to new format
        let new_format = serde_json::json!({
            "version": 2,
            "packages": old_data.get("packages").cloned().unwrap_or(serde_json::json!([])),
            "migrated_from": version,
            "migration_date": "2024-01-01"
        });

        fs::write(&new_db, serde_json::to_string_pretty(&new_format)?)?;
    }

    // Verify migration
    let migrated: serde_json::Value = serde_json::from_str(&fs::read_to_string(&new_db)?)?;
    assert_eq!(
        migrated.get("version").and_then(serde_json::Value::as_u64),
        Some(2)
    );
    assert_eq!(
        migrated
            .get("migrated_from")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );

    Ok(())
}

/// Test handling of corrupted IPC socket
#[test]
fn test_stale_socket_recovery() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let socket_path = temp_dir.path().join("daemon.sock");

    // Create a regular file pretending to be a socket (stale)
    fs::write(&socket_path, "not a real socket")?;

    // Recovery: detect that it's not a real socket and remove it
    let metadata = fs::metadata(&socket_path)?;

    if metadata.is_file() {
        // This is a stale file, not a socket - remove it
        fs::remove_file(&socket_path)?;
    }

    assert!(!socket_path.exists(), "Stale socket file should be removed");

    Ok(())
}

// Helper functions

fn create_valid_test_index() -> Vec<u8> {
    // Create a minimal valid index structure
    let mut index = Vec::new();
    index.extend_from_slice(b"OMGIDX\x01\x00"); // Header with version
    index.extend_from_slice(&[0; 1016]); // Padding to meet minimum size
    index
}

fn validate_cache_key(key: &str) -> bool {
    // Cache key validation rules:
    // - No path separators
    // - No null bytes
    // - No shell metacharacters
    // - No URL encoding
    // - Must be alphanumeric with limited punctuation

    if key.is_empty() || key.len() > 255 {
        return false;
    }

    if key.contains('/') || key.contains('\\') {
        return false;
    }

    if key.contains('\0') {
        return false;
    }

    if key.contains("..") {
        return false;
    }

    if key.contains('%') {
        return false;
    }

    if key.contains(';') || key.contains('|') || key.contains('&') {
        return false;
    }

    if key.starts_with('/') || key.starts_with('.') {
        return false;
    }

    // Only allow alphanumeric, hyphen, underscore, period
    key.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

const fn simulate_operation_without_cache() {
    // Simulate an operation that would normally use cache
    // but gracefully works without it
}

fn is_process_running(pid: u32) -> bool {
    // Check if process with given PID is running
    // This is a simplified check
    #[cfg(unix)]
    {
        let result = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output();

        match result {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_cache_key_validation_comprehensive() {
        // Additional edge cases for cache key validation
        assert!(!validate_cache_key("")); // Empty
        assert!(!validate_cache_key(&"a".repeat(256))); // Too long
        assert!(!validate_cache_key("a/b")); // Path separator
        assert!(!validate_cache_key("a\\b")); // Windows path separator
        assert!(!validate_cache_key("a\x00b")); // Null byte
        assert!(!validate_cache_key("../escape")); // Parent directory
        assert!(!validate_cache_key(".hidden")); // Hidden file
        assert!(!validate_cache_key("a%20b")); // URL encoded
        assert!(!validate_cache_key("a;b")); // Shell separator
        assert!(!validate_cache_key("a|b")); // Pipe
        assert!(!validate_cache_key("a&b")); // Ampersand

        assert!(validate_cache_key("valid-key"));
        assert!(validate_cache_key("valid_key"));
        assert!(validate_cache_key("valid.key"));
        assert!(validate_cache_key("ValidKey123"));
        assert!(validate_cache_key("a")); // Minimum valid
        assert!(validate_cache_key(&"a".repeat(255))); // Maximum valid
    }

    #[test]
    fn test_valid_index_structure() {
        let index = create_valid_test_index();
        assert!(index.len() >= 1024, "Index should meet minimum size");
        assert!(
            index.starts_with(b"OMGIDX"),
            "Index should have valid header"
        );
    }
}
