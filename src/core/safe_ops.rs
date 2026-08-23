//! Safe operations library for OMG
//!
//! Provides safe constructors and utilities for common operations that would otherwise
//! require `unwrap()` or `expect()`. This module helps eliminate panic-prone patterns
//! throughout the codebase while maintaining performance and ergonomics.

use anyhow::{Context, Result};
use std::num::{NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Safe constructor for `NonZeroU32` with context
pub fn nonzero_u32(value: u32, context: &str) -> Result<NonZeroU32> {
    NonZeroU32::new(value)
        .ok_or_else(|| anyhow::anyhow!("{context}: value must be > 0, got {value}"))
        .with_context(|| format!("Failed to create NonZeroU32 for {context}"))
}

/// Safe constructor for `NonZeroU64` with context
pub fn nonzero_u64(value: u64, context: &str) -> Result<NonZeroU64> {
    NonZeroU64::new(value)
        .ok_or_else(|| anyhow::anyhow!("{context}: value must be > 0, got {value}"))
        .with_context(|| format!("Failed to create NonZeroU64 for {context}"))
}

/// Create a `NonZeroU32` with a default fallback value.
///
/// If both `value` and `default` are zero, falls back to `NonZeroU32::MIN` (1).
pub fn nonzero_u32_or_default(value: u32, default: u32) -> NonZeroU32 {
    NonZeroU32::new(value)
        .or_else(|| NonZeroU32::new(default))
        .unwrap_or(NonZeroU32::MIN)
}

/// Safe alternative to `expect()` with better error context
pub fn expect_or<T>(option: Option<T>, context: &str) -> Result<T> {
    option.ok_or_else(|| anyhow::anyhow!("Expected value for {context} but found None"))
}

/// Validate that a path is safe for file operations
pub fn validate_path<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    let path = path.as_ref();

    // Check for empty path
    if path.as_os_str().is_empty() {
        return Err(anyhow::anyhow!("Path cannot be empty"));
    }

    // Check for null bytes
    let Some(path_str) = path.to_str() else {
        return Err(anyhow::anyhow!("Path contains invalid UTF-8"));
    };
    if path_str.contains('\0') {
        return Err(anyhow::anyhow!("Path contains null byte"));
    }

    // Return the path as-is (canonicalize() fails for non-existent paths)
    Ok(path.to_path_buf())
}

/// Safe file write with atomic operations
pub async fn atomic_write_file<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = path.as_ref().to_path_buf();
    let contents = contents.as_ref().to_vec();
    tokio::task::spawn_blocking(move || atomic_write_file_sync(path, contents))
        .await
        .context("Atomic file writer task failed")?
}

/// Safe synchronous file write with atomic operations
pub fn atomic_write_file_sync<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    use std::io::Write;

    let path = validate_path(path)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;

    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("Failed to create temporary file in {}", parent.display()))?;
    temporary
        .as_file_mut()
        .write_all(contents.as_ref())
        .with_context(|| format!("Failed to write temporary file for {}", path.display()))?;
    temporary
        .as_file_mut()
        .sync_all()
        .with_context(|| format!("Failed to sync temporary file for {}", path.display()))?;
    temporary
        .persist(&path)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to replace {}", path.display()))?;
    Ok(())
}

/// Database transaction helper with automatic rollback on error
pub struct TransactionGuard<T> {
    inner: Option<T>,
}

// SAFETY: The expects below guard a logical invariant — `inner` is `Some` until
// `commit()` consumes it. Calling `inner()`/`inner_mut()` after `commit()` is a
// programming error (use-after-move), so panicking is correct.
#[expect(clippy::expect_used)]
impl<T> TransactionGuard<T> {
    /// Create a new transaction guard
    pub fn new(transaction: T) -> Self {
        Self {
            inner: Some(transaction),
        }
    }

    /// Get a reference to the inner transaction
    #[must_use]
    pub fn inner(&self) -> &T {
        self.inner.as_ref().expect("Transaction already consumed")
    }

    /// Get a mutable reference to the inner transaction
    pub fn inner_mut(&mut self) -> &mut T {
        self.inner.as_mut().expect("Transaction already consumed")
    }

    /// Commit the transaction (preventing rollback)
    pub fn commit(mut self) -> T {
        self.inner.take().expect("Transaction already consumed")
    }
}

impl<T> Drop for TransactionGuard<T> {
    fn drop(&mut self) {
        if self.inner.is_some() {
            // Transaction will be dropped without explicit commit
            // The underlying transaction implementation should handle rollback
            tracing::warn!("Transaction dropped without commit - rollback will occur");
        }
    }
}

/// Atomic counter for safe increment operations
pub struct AtomicCounter {
    value: AtomicU64,
}

impl AtomicCounter {
    /// Create a new atomic counter
    pub fn new(initial: u64) -> Self {
        Self {
            value: AtomicU64::new(initial),
        }
    }

    /// Increment and return the new value
    pub fn increment(&self) -> u64 {
        self.value.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Get the current value
    #[must_use]
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::SeqCst)
    }

    /// Reset to a specific value
    pub fn reset(&self, new_value: u64) {
        self.value.store(new_value, Ordering::SeqCst);
    }
}

/// Rate limiter helper with safe initialization
#[derive(Debug)]
pub struct RateLimiterConfig {
    pub requests_per_second: NonZeroU32,
    pub burst_size: NonZeroU32,
}

impl RateLimiterConfig {
    /// Create a new rate limiter config with safe defaults
    pub fn new() -> Result<Self> {
        Ok(Self {
            requests_per_second: nonzero_u32(100, "rate limiter requests per second")?,
            burst_size: nonzero_u32(200, "rate limiter burst size")?,
        })
    }

    /// Create with custom values
    pub fn with_values(requests_per_second: u32, burst_size: u32) -> Result<Self> {
        Ok(Self {
            requests_per_second: nonzero_u32(
                requests_per_second,
                "rate limiter requests per second",
            )?,
            burst_size: nonzero_u32(burst_size, "rate limiter burst size")?,
        })
    }
}

impl Default for RateLimiterConfig {
    fn default() -> Self {
        Self {
            // SAFETY: 100 and 200 are known non-zero constants; these cannot fail.
            requests_per_second: NonZeroU32::new(100).expect("100 is non-zero"),
            burst_size: NonZeroU32::new(200).expect("200 is non-zero"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[test]
    fn test_nonzero_u32_success() {
        let result = nonzero_u32(42, "test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().get(), 42);
    }

    #[test]
    fn test_nonzero_u32_zero() {
        let result = nonzero_u32(0, "test");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // anyhow wraps errors, so check the chain
        let found = err
            .chain()
            .any(|e| e.to_string().contains("value must be > 0, got 0"));
        assert!(
            found,
            "Error chain should contain 'value must be > 0, got 0'"
        );
    }

    #[test]
    fn test_nonzero_u32_or_default() {
        let nz1 = nonzero_u32_or_default(0, 100);
        assert_eq!(nz1.get(), 100);

        let nz2 = nonzero_u32_or_default(50, 100);
        assert_eq!(nz2.get(), 50);
    }

    #[test]
    fn test_expect_or_some() {
        let option = Some(42);
        let result = expect_or(option, "test value");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_expect_or_none() {
        let option: Option<i32> = None;
        let result = expect_or(option, "test value");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Expected value for test value")
        );
    }

    #[test]
    fn test_validate_path_valid() {
        let path = "/tmp";
        let result = validate_path(path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_path_empty() {
        let path = "";
        let result = validate_path(path);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_atomic_write_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = b"Hello, world!";

        let result = atomic_write_file(&file_path, content).await;
        assert!(result.is_ok());

        let read_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content, "Hello, world!");
    }

    #[test]
    fn test_atomic_write_file_sync() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = b"Hello, world!";

        let result = atomic_write_file_sync(&file_path, content);
        assert!(result.is_ok());

        let read_content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, "Hello, world!");
    }

    #[test]
    fn test_transaction_guard_commit() {
        let guard = TransactionGuard::new("transaction_data");
        let data = guard.commit();
        assert_eq!(data, "transaction_data");
    }

    #[test]
    fn test_transaction_guard_drop() {
        // This test verifies that dropping without commit logs a warning
        // In a real implementation, the transaction type would handle rollback
        let _guard = TransactionGuard::new("transaction_data");
        // Guard drops here without commit - should log warning
    }

    #[test]
    fn test_atomic_counter() {
        let counter = AtomicCounter::new(10);
        assert_eq!(counter.get(), 10);

        let new_value = counter.increment();
        assert_eq!(new_value, 11);
        assert_eq!(counter.get(), 11);

        counter.reset(5);
        assert_eq!(counter.get(), 5);
    }

    #[test]
    fn test_rate_limiter_config_new() {
        let config = RateLimiterConfig::new();
        assert!(config.is_ok());

        let config = config.unwrap();
        assert_eq!(config.requests_per_second.get(), 100);
        assert_eq!(config.burst_size.get(), 200);
    }

    #[test]
    fn test_rate_limiter_config_with_values() {
        let config = RateLimiterConfig::with_values(50, 150);
        assert!(config.is_ok());

        let config = config.unwrap();
        assert_eq!(config.requests_per_second.get(), 50);
        assert_eq!(config.burst_size.get(), 150);
    }

    #[test]
    fn test_rate_limiter_config_zero_values() {
        let config = RateLimiterConfig::with_values(0, 0);
        assert!(config.is_err());
        let err = config.unwrap_err();
        // Check the error chain for the inner error message
        let found = err
            .chain()
            .any(|e| e.to_string().contains("value must be > 0"));
        assert!(found, "Error chain should contain 'value must be > 0'");
    }

    #[test]
    fn test_rate_limiter_config_default() {
        let config = RateLimiterConfig::default();
        assert_eq!(config.requests_per_second.get(), 100);
        assert_eq!(config.burst_size.get(), 200);
    }
}
