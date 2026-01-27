//! File watcher for automatic cache invalidation
//!
//! Watches /var/lib/apt/lists/ and /var/lib/dpkg/status for changes
//! and invalidates the Debian package cache when updates occur.
//!
//! This is used in daemon mode for reactive cache updates.

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

/// Flag indicating the Debian index needs refresh
static DEBIAN_INDEX_DIRTY: AtomicBool = AtomicBool::new(false);

/// Flag indicating dpkg status needs refresh
static DPKG_STATUS_DIRTY: AtomicBool = AtomicBool::new(false);

/// Check if the Debian index is dirty and needs refresh
#[inline]
pub fn is_debian_index_dirty() -> bool {
    DEBIAN_INDEX_DIRTY.load(Ordering::Relaxed)
}

/// Check if dpkg status is dirty and needs refresh
#[inline]
pub fn is_dpkg_status_dirty() -> bool {
    DPKG_STATUS_DIRTY.load(Ordering::Relaxed)
}

/// Clear the dirty flag after refresh
#[inline]
pub fn clear_debian_index_dirty() {
    DEBIAN_INDEX_DIRTY.store(false, Ordering::Relaxed);
}

/// Clear the dpkg status dirty flag after refresh
#[inline]
pub fn clear_dpkg_status_dirty() {
    DPKG_STATUS_DIRTY.store(false, Ordering::Relaxed);
}

/// File watcher handle - drop to stop watching
pub struct DebianFileWatcher {
    _watcher: RecommendedWatcher,
    /// Shutdown channel to signal task termination
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Background task handle for clean shutdown
    task_handle: Option<tokio::task::JoinHandle<()>>,
}

impl DebianFileWatcher {
    /// Start watching Debian package files for changes
    ///
    /// Returns a watcher handle. Drop the handle to stop watching.
    pub fn start() -> notify::Result<Self> {
        let (tx, mut rx) = mpsc::channel(100);

        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<Event>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            Config::default()
                .with_poll_interval(Duration::from_secs(2))
                .with_compare_contents(false),
        )?;

        // Watch apt lists directory
        let apt_lists = Path::new("/var/lib/apt/lists");
        if apt_lists.exists() {
            watcher.watch(apt_lists, RecursiveMode::NonRecursive)?;
        }

        // Watch dpkg status file
        let dpkg_status = Path::new("/var/lib/dpkg/status");
        if dpkg_status.exists() {
            watcher.watch(dpkg_status, RecursiveMode::NonRecursive)?;
        }

        // Create shutdown channel for clean task termination
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        // Spawn event handler task with shutdown support
        let task_handle = tokio::spawn(async move {
            let mut debounce_apt = tokio::time::Instant::now();
            let mut debounce_dpkg = tokio::time::Instant::now();
            let debounce_duration = Duration::from_secs(1);

            loop {
                tokio::select! {
                    // Shutdown signal received
                    _ = &mut shutdown_rx => {
                        tracing::debug!("File watcher shutting down gracefully");
                        break;
                    }
                    // File system event received
                    Some(event) = rx.recv() => {
                        // Only care about modify/create/delete events
                        match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {}
                            _ => continue,
                        }

                        for path in &event.paths {
                            let path_str = path.to_string_lossy();

                            // Check if it's an apt lists change
                            if path_str.contains("/var/lib/apt/lists") {
                                let now = tokio::time::Instant::now();
                                if now.duration_since(debounce_apt) > debounce_duration {
                                    debounce_apt = now;
                                    DEBIAN_INDEX_DIRTY.store(true, Ordering::Relaxed);
                                    tracing::debug!("Apt lists changed, marking index dirty");
                                }
                            }

                            // Check if it's a dpkg status change
                            if path_str.contains("/var/lib/dpkg/status") {
                                let now = tokio::time::Instant::now();
                                if now.duration_since(debounce_dpkg) > debounce_duration {
                                    debounce_dpkg = now;
                                    DPKG_STATUS_DIRTY.store(true, Ordering::Relaxed);
                                    tracing::debug!("Dpkg status changed, marking dirty");
                                }
                            }
                        }
                    }
                    // Channel closed (all senders dropped)
                    else => {
                        tracing::debug!("File watcher channel closed, terminating");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            _watcher: watcher,
            shutdown_tx: Some(shutdown_tx),
            task_handle: Some(task_handle),
        })
    }
}

impl Drop for DebianFileWatcher {
    fn drop(&mut self) {
        // Send shutdown signal to background task
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
            tracing::debug!("Sent shutdown signal to file watcher task");
        }

        // Wait for task to complete gracefully (non-blocking)
        if let Some(handle) = self.task_handle.take() {
            // Try to get a runtime handle to block on task completion
            // If we're in a tokio runtime, block until task finishes
            // If not, the task will be aborted when the handle is dropped
            if let Ok(runtime_handle) = tokio::runtime::Handle::try_current() {
                if let Ok(result) = runtime_handle.block_on(async {
                    tokio::time::timeout(Duration::from_secs(5), handle).await
                }) {
                    if let Err(e) = result {
                        tracing::warn!("File watcher task failed during shutdown: {e}");
                    } else {
                        tracing::debug!("File watcher task shut down cleanly");
                    }
                } else {
                    tracing::warn!("File watcher task did not shut down within 5 seconds");
                }
            } else {
                // Outside tokio runtime - task will be aborted when handle drops
                tracing::debug!("File watcher task will terminate asynchronously");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirty_flags() {
        // Initially not dirty
        clear_debian_index_dirty();
        clear_dpkg_status_dirty();

        assert!(!is_debian_index_dirty());
        assert!(!is_dpkg_status_dirty());

        // Mark dirty
        DEBIAN_INDEX_DIRTY.store(true, Ordering::Relaxed);
        assert!(is_debian_index_dirty());

        // Clear
        clear_debian_index_dirty();
        assert!(!is_debian_index_dirty());
    }
}
