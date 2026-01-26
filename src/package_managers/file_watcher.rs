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

        // Spawn event handler task
        tokio::spawn(async move {
            let mut debounce_apt = tokio::time::Instant::now();
            let mut debounce_dpkg = tokio::time::Instant::now();
            let debounce_duration = Duration::from_secs(1);

            while let Some(event) = rx.recv().await {
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
        });

        Ok(Self { _watcher: watcher })
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
