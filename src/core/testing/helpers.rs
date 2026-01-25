//! Test helpers and utilities

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;
use tempfile::TempDir;

/// Test context builder for setting up isolated test environments
#[allow(clippy::struct_field_names)]
pub struct TestContext {
    temp_dir: Option<TempDir>,
    data_dir: PathBuf,
    config_dir: PathBuf,
    cache_dir: PathBuf,
}

impl TestContext {
    /// Create a new test context with temporary directories
    pub fn new() -> Self {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        let data_dir = base.join("data");
        let config_dir = base.join("config");
        let cache_dir = base.join("cache");

        fs::create_dir_all(&data_dir).unwrap();
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&cache_dir).unwrap();

        Self {
            temp_dir: Some(temp_dir),
            data_dir,
            config_dir,
            cache_dir,
        }
    }

    /// Get the data directory path
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Get the config directory path
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Get the cache directory path
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Create a test file in the data directory
    pub fn create_test_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.data_dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    /// Create a test file in the config directory
    pub fn create_config_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.config_dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    /// Read a test file
    pub fn read_file(&self, path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    /// Check if a file exists
    pub fn file_exists(&self, path: &Path) -> bool {
        path.exists()
    }
}

impl Default for TestContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // TempDir will be cleaned up automatically
        self.temp_dir.take();
    }
}

/// Async test helper for timeout operations
pub async fn with_timeout<F, T>(duration: Duration, f: F) -> anyhow::Result<T>
where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    tokio::time::timeout(duration, f)
        .await
        .map_err(|_| anyhow::anyhow!("Operation timed out after {duration:?}"))?
}

/// Retry helper for flaky tests
pub async fn retry<F, T, E>(max_attempts: usize, delay: Duration, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Pin<Box<dyn Future<Output = Result<T, E>> + Send>>,
    E: std::fmt::Display,
{
    let mut last_error = None;

    for attempt in 1..=max_attempts {
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = Some(e);
                if attempt < max_attempts {
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    Err(last_error.unwrap())
}

/// Helper to run async tests with a timeout
#[macro_export]
macro_rules! async_test_timeout {
    ($test_name:ident, $timeout:expr, $test_body:block) => {
        #[tokio::test]
        async fn $test_name() {
            tokio::time::timeout(std::time::Duration::from_millis($timeout), async {
                $test_body
            })
            .await
            .expect(&format!("Test {} timed out", stringify!($test_name)));
        }
    };
}

/// Helper to assert that an operation returns an error
#[macro_export]
macro_rules! assert_err {
    ($result:expr, $pat:pat) => {
        match $result {
            Err($pat) => (),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    };
    ($result:expr) => {
        match $result {
            Err(_) => (),
            Ok(_) => panic!("Expected error but got Ok"),
        }
    };
}

/// Helper to assert that an operation returns Ok with a value
#[macro_export]
macro_rules! assert_ok {
    ($result:expr) => {
        match $result {
            Ok(_) => (),
            Err(e) => panic!("Expected Ok but got error: {}", e),
        }
    };
    ($result:expr, $pat:pat) => {
        match $result {
            Ok($pat) => (),
            Err(e) => panic!("Expected Ok({}) but got error: {}", stringify!($pat), e),
        }
    };
}

/// Utility for measuring execution time
pub struct Timer {
    start: std::time::Instant,
}

impl Timer {
    /// Start a new timer
    pub fn start() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }

    /// Get the elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Get the elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u128 {
        self.elapsed().as_millis()
    }

    /// Assert that the elapsed time is less than a threshold
    pub fn assert_less_than(&self, threshold: Duration) {
        let elapsed = self.elapsed();
        assert!(
            elapsed < threshold,
            "Operation took {elapsed:?}, expected less than {threshold:?}"
        );
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::start()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = TestContext::new();
        assert!(ctx.data_dir().exists());
        assert!(ctx.config_dir().exists());
        assert!(ctx.cache_dir().exists());
    }

    #[test]
    fn test_context_file_operations() {
        let ctx = TestContext::new();
        let path = ctx.create_test_file("test.txt", "hello world");
        assert!(ctx.file_exists(&path));
        assert_eq!(ctx.read_file(&path), "hello world");
    }

    #[test]
    fn test_timer() {
        let timer = Timer::start();
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = timer.elapsed_ms();
        assert!(elapsed >= 10);
        timer.assert_less_than(Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_with_timeout() {
        let result = with_timeout(Duration::from_millis(100), async {
            Ok::<(), anyhow::Error>(())
        })
        .await;
        assert!(result.is_ok());

        let result = with_timeout(Duration::from_millis(10), async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok::<(), anyhow::Error>(())
        })
        .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retry_success() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry(3, Duration::from_millis(10), || {
            let attempts_clone = attempts_clone.clone();
            Box::pin(async move {
                let count = attempts_clone.fetch_add(1, Ordering::SeqCst) + 1;
                if count < 3 {
                    Err(String::from("not yet"))
                } else {
                    Ok("success")
                }
            })
        })
        .await;

        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_failure() {
        let result = retry(2, Duration::from_millis(10), || {
            Box::pin(async move { Err::<(), String>(String::from("always fails")) })
        })
        .await;

        assert!(result.is_err());
    }
}
