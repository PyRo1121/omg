//! Sudoloop: Keep sudo credentials alive during long operations
//!
//! Similar to yay's --sudoloop, this runs a background task that
//! periodically runs `sudo -v` to refresh the sudo timestamp.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::sleep;

/// Handle for a running sudoloop
pub struct SudoLoop {
    running: Arc<AtomicBool>,
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl SudoLoop {
    /// Start a sudoloop that refreshes sudo credentials every 60 seconds
    pub fn start() -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let handle = tokio::spawn(async move {
            tracing::debug!("Sudoloop started");

            // Wait 60s before first refresh (sudo timestamp is ~5 minutes)
            sleep(Duration::from_secs(60)).await;

            while running_clone.load(Ordering::Relaxed) {
                // Refresh sudo timestamp with -v (validate, extend timeout)
                let result = Command::new("sudo")
                    .arg("-v")
                    .output()
                    .await;

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

                // Wait another 60s before next refresh
                sleep(Duration::from_secs(60)).await;
            }

            tracing::debug!("Sudoloop stopped");
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    /// Stop the sudoloop
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

impl Drop for SudoLoop {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Check if we can use sudoloop (sudo is available and we're not root)
pub fn can_use_sudoloop() -> bool {
    !crate::core::is_root() && which::which("sudo").is_ok()
}
