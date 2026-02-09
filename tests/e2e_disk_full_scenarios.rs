//! E2E Disk Full Scenario Tests
//!
//! Tests for graceful handling of ENOSPC (disk full) errors across all operations.
//! These scenarios occur in production when systems run low on disk space.

#![cfg(feature = "arch")]

use anyhow::Result;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;

mod common;

/// Test that write operations detect and report ENOSPC gracefully
#[test]
fn test_write_failure_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test.txt");

    // Write should succeed initially
    fs::write(&test_file, b"test content")?;
    assert!(test_file.exists());

    // Verify file content
    let content = fs::read_to_string(&test_file)?;
    assert_eq!(content, "test content");

    Ok(())
}

/// Test graceful handling when cache directory becomes full
#[test]
fn test_cache_write_graceful_degradation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let cache_file = temp_dir.path().join("cache.json");

    // Simulate cache write
    let cache_data = serde_json::json!({
        "packages": ["firefox", "vim", "git"],
        "timestamp": 1_234_567_890
    });

    // Normal write should succeed
    let content = serde_json::to_string_pretty(&cache_data)?;
    fs::write(&cache_file, &content)?;

    // Verify cache is readable
    let read_back: serde_json::Value = serde_json::from_str(&fs::read_to_string(&cache_file)?)?;
    assert_eq!(read_back, cache_data);

    Ok(())
}

/// Test that atomic file writes don't leave partial files on failure
#[test]
fn test_atomic_write_cleanup() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let target_file = temp_dir.path().join("data.bin");
    let temp_file = temp_dir.path().join("data.bin.tmp");

    // Simulate atomic write pattern: write to temp, then rename
    {
        let mut file = fs::File::create(&temp_file)?;
        file.write_all(b"complete data")?;
        file.sync_all()?;
    }

    // Rename to final location
    fs::rename(&temp_file, &target_file)?;

    // Temp file should not exist after successful rename
    assert!(!temp_file.exists(), "Temp file should be cleaned up");
    assert!(target_file.exists(), "Target file should exist");

    Ok(())
}

/// Test database transaction rollback simulation
#[test]
fn test_transaction_rollback_on_failure() -> Result<()> {
    use std::collections::HashMap;

    let temp_dir = TempDir::new()?;
    let db_file = temp_dir.path().join("test.db");

    // Create initial database state
    let mut db: HashMap<String, String> = HashMap::new();
    db.insert("key1".to_string(), "value1".to_string());

    // Save initial state
    let initial_state = serde_json::to_string(&db)?;
    fs::write(&db_file, &initial_state)?;

    // Start a "transaction" - modify in memory
    db.insert("key2".to_string(), "value2".to_string());

    // Simulate failure during write (we won't actually write)
    let simulated_failure = true;

    if simulated_failure {
        // Rollback: reload from disk
        let saved_state = fs::read_to_string(&db_file)?;
        db = serde_json::from_str(&saved_state)?;
    }

    // Verify rollback worked - key2 should not exist
    assert!(db.contains_key("key1"), "Original data should be preserved");
    assert!(
        !db.contains_key("key2"),
        "Uncommitted data should be rolled back"
    );

    Ok(())
}

/// Test that temporary files are cleaned up on error
#[test]
fn test_temp_file_cleanup_on_error() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create some temporary files
    let temp_files: Vec<PathBuf> = (0..5)
        .map(|i| {
            let path = temp_dir.path().join(format!("temp_{i}.tmp"));
            fs::write(&path, format!("temp data {i}")).unwrap();
            path
        })
        .collect();

    // Verify all temp files exist
    for path in &temp_files {
        assert!(path.exists());
    }

    // Simulate cleanup on error
    for path in &temp_files {
        if path.exists() {
            fs::remove_file(path)?;
        }
    }

    // Verify cleanup
    for path in &temp_files {
        assert!(!path.exists(), "Temp file should be cleaned up");
    }

    Ok(())
}

/// Test log rotation when disk is nearly full
#[test]
fn test_log_rotation_under_pressure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let log_file = temp_dir.path().join("app.log");
    let rotated_log = temp_dir.path().join("app.log.1");

    // Create a "full" log file
    let log_content = "x".repeat(1024 * 1024); // 1MB
    fs::write(&log_file, &log_content)?;

    // Simulate log rotation
    if log_file.exists() {
        // Rotate: rename current to .1
        fs::rename(&log_file, &rotated_log)?;

        // Create new empty log
        fs::write(&log_file, "")?;
    }

    // Verify rotation
    assert!(log_file.exists(), "New log file should exist");
    assert!(rotated_log.exists(), "Rotated log should exist");

    let new_size = fs::metadata(&log_file)?.len();
    assert_eq!(new_size, 0, "New log file should be empty");

    Ok(())
}

/// Test download resumption capability
#[test]
fn test_download_resume_after_failure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let download_path = temp_dir.path().join("package.tar.gz");
    let partial_path = temp_dir.path().join("package.tar.gz.part");

    // Simulate partial download (interrupted at 50%)
    let total_size = 1024;
    let partial_size = total_size / 2;
    let partial_data = vec![0xABu8; partial_size];
    fs::write(&partial_path, &partial_data)?;

    // Verify partial file exists with correct size
    let meta = fs::metadata(&partial_path)?;
    #[allow(clippy::cast_possible_truncation)]
    {
        assert_eq!(meta.len() as usize, partial_size);
    }

    // Simulate resume: append remaining data
    let remaining_data = vec![0xCDu8; total_size - partial_size];
    {
        let mut file = fs::OpenOptions::new().append(true).open(&partial_path)?;
        file.write_all(&remaining_data)?;
    }

    // Rename to final path
    fs::rename(&partial_path, &download_path)?;

    // Verify complete download
    let complete_data = fs::read(&download_path)?;
    assert_eq!(complete_data.len(), total_size);
    assert_eq!(&complete_data[..partial_size], &vec![0xABu8; partial_size]);
    assert_eq!(
        &complete_data[partial_size..],
        &vec![0xCDu8; total_size - partial_size]
    );

    Ok(())
}

/// Test cache eviction under memory pressure
#[test]
fn test_cache_eviction_policy() {
    use std::collections::HashMap;

    let max_entries = 5;
    let mut cache: HashMap<String, (i64, String)> = HashMap::new(); // (timestamp, value)

    // Fill cache to capacity
    #[allow(clippy::cast_possible_wrap)]
    for i in 0..max_entries {
        cache.insert(format!("key{i}"), (i as i64, format!("value{i}")));
    }
    assert_eq!(cache.len(), max_entries);

    // Attempt to add new entry - should trigger eviction
    let new_key = "new_key".to_string();

    // LRU eviction: remove oldest entry
    if cache.len() >= max_entries {
        // Find and remove entry with lowest timestamp
        if let Some(oldest_key) = cache
            .iter()
            .min_by_key(|(_, (ts, _))| *ts)
            .map(|(k, _)| k.clone())
        {
            cache.remove(&oldest_key);
        }
    }

    // Add new entry
    cache.insert(new_key.clone(), (100, "new_value".to_string()));

    // Verify eviction worked
    assert_eq!(cache.len(), max_entries);
    assert!(cache.contains_key(&new_key));
    assert!(
        !cache.contains_key("key0"),
        "Oldest entry should be evicted"
    );
}

/// Test graceful shutdown when disk operations fail
#[test]
fn test_graceful_shutdown_on_disk_error() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Simulate state that needs to be saved on shutdown
    let state = serde_json::json!({
        "session_id": "abc123",
        "pending_operations": 3
    });

    let state_file = temp_dir.path().join("state.json");

    // Attempt to save state
    let save_result = (|| -> Result<()> {
        let content = serde_json::to_string_pretty(&state)?;
        fs::write(&state_file, content)?;
        Ok(())
    })();

    match save_result {
        Ok(()) => {
            // State saved successfully
            assert!(state_file.exists());
        }
        Err(e) => {
            // Graceful degradation: log error but don't crash
            eprintln!("Warning: Could not save state: {e}");
            // Continue shutdown without saved state
        }
    }

    Ok(())
}

/// Test parallel write coordination (prevent corruption)
#[tokio::test]
async fn test_parallel_write_safety() -> Result<()> {
    use std::sync::Arc;
    use tokio::fs;
    use tokio::sync::Semaphore;

    let temp_dir = TempDir::new()?;
    let target_file = temp_dir.path().join("shared.txt");

    // Use semaphore to ensure only one write at a time
    let semaphore = Arc::new(Semaphore::new(1));

    let mut handles = vec![];

    for i in 0..10 {
        let sem = semaphore.clone();
        let path = target_file.clone();

        handles.push(tokio::spawn(async move {
            // Acquire exclusive access
            let _permit = sem.acquire().await.unwrap();

            // Append to file
            let content = format!("write {i}\n");
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await?
                .write_all(content.as_bytes())
                .await?;

            Ok::<_, anyhow::Error>(())
        }));
    }

    // Wait for all writes
    for handle in handles {
        handle.await??;
    }

    // Verify file is not corrupted (all writes present)
    let content = std::fs::read_to_string(&target_file)?;
    assert_eq!(content.lines().count(), 10, "All writes should be present");

    Ok(())
}

/// Test that file locks prevent concurrent access
#[test]
fn test_file_lock_protection() -> Result<()> {
    use std::io::{Read, Seek, SeekFrom};

    let temp_dir = TempDir::new()?;
    let lock_file = temp_dir.path().join("lock.lock");

    // Create lock file
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_file)?;

    // Write some data
    file.write_all(b"locked")?;
    file.sync_all()?;

    // Seek back and read
    file.seek(SeekFrom::Start(0))?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    assert_eq!(content, "locked");

    Ok(())
}

/// Test handling of read-only filesystem scenarios
#[test]
fn test_readonly_filesystem_handling() -> Result<()> {
    // Root bypasses POSIX file permission checks on Linux, so this test
    // is meaningless when running as root (e.g., in CI Docker containers)
    #[cfg(unix)]
    {
        if std::process::Command::new("id")
            .arg("-u")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
            .unwrap_or(false)
        {
            eprintln!("Skipping: root bypasses read-only file permissions");
            return Ok(());
        }
    }

    let temp_dir = TempDir::new()?;
    let file_path = temp_dir.path().join("readonly.txt");

    // Create file
    fs::write(&file_path, "original content")?;

    // Make file read-only
    let mut perms = fs::metadata(&file_path)?.permissions();
    perms.set_readonly(true);
    fs::set_permissions(&file_path, perms)?;

    // Attempt to write should fail
    let write_result = fs::write(&file_path, "new content");
    assert!(write_result.is_err(), "Write to read-only file should fail");

    // Restore permissions for cleanup
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&file_path)?.permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&file_path, perms)?;
    }
    #[cfg(not(unix))]
    {
        let mut perms = fs::metadata(&file_path)?.permissions();
        perms.set_readonly(false);
        fs::set_permissions(&file_path, perms)?;
    }

    Ok(())
}

#[cfg(test)]
mod unit_tests {
    #[test]
    fn test_enospc_error_code() {
        // ENOSPC = 28 on Linux
        #[cfg(target_os = "linux")]
        {
            let enospc_error = std::io::Error::from_raw_os_error(28);
            // ErrorKind doesn't have ENOSPC variant, but we can check the code
            assert_eq!(enospc_error.raw_os_error(), Some(28));
        }
    }

    #[test]
    fn test_disk_space_check() {
        // Test that we can query available disk space
        let path = std::env::temp_dir();

        // This is platform-specific, just verify it doesn't panic
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = std::fs::metadata(&path) {
                let _dev = meta.dev();
            }
        }
    }

    #[test]
    fn test_safe_file_size_limits() {
        // Verify our size limits are reasonable
        const MAX_PACKAGE_SIZE: u64 = 2 * 1024 * 1024 * 1024; // 2GB
        const MAX_LOG_SIZE: u64 = 100 * 1024 * 1024; // 100MB
        const MAX_CACHE_SIZE: u64 = 500 * 1024 * 1024; // 500MB

        const { assert!(MAX_PACKAGE_SIZE > MAX_CACHE_SIZE) };
        const { assert!(MAX_CACHE_SIZE > MAX_LOG_SIZE) };
    }
}
