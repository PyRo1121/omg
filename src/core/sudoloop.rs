//! Sudoloop: Keep sudo credentials alive during long operations
//!
//! Similar to yay's --sudoloop, this runs a background task that
//! periodically runs `sudo -v` to refresh the sudo timestamp.
//!
//! ## How it works
//!
//! 1. When started, the loop waits 60 seconds before the first refresh
//!    (since sudo timestamp is typically 5 minutes by default)
//! 2. Every 60 seconds thereafter, it runs `sudo -v` to extend the timestamp
//! 3. The loop automatically stops when dropped (RAII pattern)
//!
//! ## Example
//!
//! ```no_run
//! use omg_lib::core::sudoloop::{can_use_sudoloop, SudoLoop};
//!
//! if can_use_sudoloop() {
//!     let _sudoloop = SudoLoop::start();
//!     // ... long operation that needs sudo ...
//!     // sudoloop automatically stops when _sudoloop is dropped
//! }
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::sleep;

/// Interval between sudo credential refreshes (60 seconds)
const REFRESH_INTERVAL_SECS: u64 = 60;

/// Handle for a running sudoloop
///
/// The loop runs in a background task and will be stopped when this
/// handle is dropped.
pub struct SudoLoop {
    running: Arc<AtomicBool>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl SudoLoop {
    /// Start a sudoloop that refreshes sudo credentials periodically
    ///
    /// The loop waits 60 seconds before the first refresh, then runs
    /// `sudo -v` every 60 seconds to keep credentials alive.
    pub fn start() -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let handle = tokio::spawn(async move {
            tracing::debug!("Sudoloop started");

            // Wait before first refresh (sudo timestamp is ~5 minutes by default)
            sleep(Duration::from_secs(REFRESH_INTERVAL_SECS)).await;

            while running_clone.load(Ordering::SeqCst) {
                // Refresh sudo timestamp with -v (validate, extend timeout)
                let result = Command::new("sudo").arg("-v").output().await;

                match result {
                    Ok(output) if output.status.success() => {
                        tracing::debug!("Sudoloop: credentials refreshed");
                    }
                    Ok(output) => {
                        tracing::warn!(
                            "Sudoloop: failed to refresh credentials: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                        // Continue anyway - user might have NOPASSWD configured
                    }
                    Err(e) => {
                        tracing::warn!("Sudoloop: error running sudo -v: {e}");
                    }
                }

                // Wait before next refresh
                sleep(Duration::from_secs(REFRESH_INTERVAL_SECS)).await;
            }

            tracing::debug!("Sudoloop stopped");
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    /// Stop the sudoloop
    ///
    /// This signals the background task to stop and aborts it if needed.
    /// Called automatically when the `SudoLoop` is dropped.
    pub fn stop(&mut self) {
        // Signal the loop to stop
        self.running.store(false, Ordering::SeqCst);

        // Abort the task to ensure immediate cleanup
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }

    /// Check if the sudoloop is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Drop for SudoLoop {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Check if we can use sudoloop (sudo is available and we're not root)
///
/// Returns `false` if:
/// - Already running as root (no need for sudo)
/// - sudo is not installed
pub fn can_use_sudoloop() -> bool {
    !crate::core::is_root() && which::which("sudo").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_use_sudoloop_not_root() {
        // This test assumes we're not running as root in the test environment
        // and that sudo is installed (typical development setup)
        let result = can_use_sudoloop();
        // Don't assert specific value since it depends on environment
        // Just verify it doesn't panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_sudoloop_start_stop() {
        let mut loop_handle = SudoLoop::start();

        // Should be running after start
        assert!(loop_handle.is_running());

        // Stop should set running to false
        loop_handle.stop();
        assert!(!loop_handle.is_running());
    }

    #[tokio::test]
    async fn test_sudoloop_drop_stops() {
        let running_flag;

        {
            let loop_handle = SudoLoop::start();
            running_flag = loop_handle.running.clone();
            assert!(running_flag.load(Ordering::SeqCst));
        }
        // After drop, should be stopped
        assert!(!running_flag.load(Ordering::SeqCst));
    }
}
