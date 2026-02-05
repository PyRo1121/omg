use std::future::Future;
use std::time::Duration;

use anyhow::Result;

const DEFAULT_MAX_RETRIES: u32 = 3;
const DEFAULT_INITIAL_BACKOFF_MS: u64 = 500;
const DEFAULT_MAX_BACKOFF_MS: u64 = 30_000;

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
    pub exponential_base: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            initial_backoff_ms: DEFAULT_INITIAL_BACKOFF_MS,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
            exponential_base: 2,
        }
    }
}

impl RetryConfig {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    #[must_use]
    pub fn with_backoff(mut self, initial_ms: u64, max_ms: u64) -> Self {
        self.initial_backoff_ms = initial_ms;
        self.max_backoff_ms = max_ms;
        self
    }

    #[doc(hidden)]
    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        let backoff_ms =
            self.initial_backoff_ms * u64::from(self.exponential_base.saturating_pow(attempt));
        let backoff_ms = backoff_ms.min(self.max_backoff_ms);
        Duration::from_millis(backoff_ms)
    }
}

pub fn is_retryable(error: &anyhow::Error) -> bool {
    let error_str = error.to_string().to_lowercase();

    error_str.contains("timeout")
        || error_str.contains("connection")
        || error_str.contains("network")
        || error_str.contains("temporary")
        || error_str.contains("try again")
        || error_str.contains("unavailable")
        || error_str.contains("timed out")
        || error_str.contains("econnrefused")
        || error_str.contains("econnreset")
        || error_str.contains("broken pipe")
}

pub async fn retry_async<F, Fut, T>(config: &RetryConfig, mut operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut last_error = None;

    for attempt in 0..=config.max_retries {
        if attempt > 0 {
            let backoff = config.calculate_backoff(attempt - 1);
            tracing::debug!("Retry attempt {} after {:?} backoff", attempt, backoff);
            tokio::time::sleep(backoff).await;
        }

        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                tracing::warn!(
                    "Operation failed (attempt {}/{}): {}",
                    attempt + 1,
                    config.max_retries + 1,
                    e
                );

                if !is_retryable(&e) || attempt == config.max_retries {
                    return Err(e);
                }

                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Operation failed after retries")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_success_on_first_attempt() {
        let config = RetryConfig::new(3);
        let result = retry_async(&config, || async { Ok::<_, anyhow::Error>(42) }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let config = RetryConfig::new(3);
        let attempt_count = Arc::new(AtomicU32::new(0));

        let result = retry_async(&config, || {
            let count = Arc::clone(&attempt_count);
            async move {
                let current = count.fetch_add(1, Ordering::SeqCst);
                if current < 2 {
                    Err(anyhow::anyhow!("network timeout"))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_fails_after_max_attempts() {
        let config = RetryConfig::new(2);
        let result = retry_async(&config, || async {
            Err::<i32, _>(anyhow::anyhow!("connection refused"))
        })
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_non_retryable_error_fails_immediately() {
        let config = RetryConfig::new(5);
        let attempt_count = Arc::new(AtomicU32::new(0));

        let result = retry_async(&config, || {
            let count = Arc::clone(&attempt_count);
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err::<i32, _>(anyhow::anyhow!("permission denied"))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_backoff_calculation() {
        let config = RetryConfig::default();

        assert_eq!(config.calculate_backoff(0), Duration::from_millis(500));
        assert_eq!(config.calculate_backoff(1), Duration::from_millis(1000));
        assert_eq!(config.calculate_backoff(2), Duration::from_millis(2000));
        assert_eq!(config.calculate_backoff(3), Duration::from_millis(4000));
    }

    #[test]
    fn test_backoff_max_cap() {
        let config = RetryConfig::default().with_backoff(1000, 5000);

        assert_eq!(config.calculate_backoff(10), Duration::from_millis(5000));
    }

    #[test]
    fn test_is_retryable() {
        assert!(is_retryable(&anyhow::anyhow!("connection timeout")));
        assert!(is_retryable(&anyhow::anyhow!("network error")));
        assert!(is_retryable(&anyhow::anyhow!("Connection refused")));
        assert!(is_retryable(&anyhow::anyhow!("Timed out")));

        assert!(!is_retryable(&anyhow::anyhow!("permission denied")));
        assert!(!is_retryable(&anyhow::anyhow!("file not found")));
        assert!(!is_retryable(&anyhow::anyhow!("invalid argument")));
    }
}
