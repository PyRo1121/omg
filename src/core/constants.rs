//! Centralized constants for OMG
//!
//! This module consolidates magic numbers, timeouts, and TTL values
//! that were previously scattered across the codebase.

use std::time::Duration;

/// HTTP request timeouts
pub mod http {
    use super::Duration;

    /// Default timeout for general HTTP requests
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

    /// Connection establishment timeout
    pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

    /// Read timeout for receiving response data
    pub const READ_TIMEOUT: Duration = Duration::from_secs(30);

    /// Timeout for large file downloads (packages, archives)
    pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(5);

    /// Connection timeout for downloads (longer for slow mirrors)
    pub const DOWNLOAD_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

    /// Read timeout for downloads (allows for slow transfer rates)
    pub const DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(60);

    /// PGP keyserver fetch timeout
    pub const KEYSERVER_TIMEOUT: Duration = Duration::from_secs(10);
}

/// Cache time-to-live values
pub mod cache_ttl {
    use super::Duration;

    /// Memory cache TTL (30 minutes) - for hot data like package lists
    pub const MEMORY: Duration = Duration::from_secs(30 * 60);

    /// Disk cache TTL (24 hours) - for persistent data
    pub const DISK: Duration = Duration::from_secs(24 * 60 * 60);

    /// Mirror list cache TTL (6 hours) - mirrors don't change frequently
    pub const MIRROR_LIST: Duration = Duration::from_secs(6 * 60 * 60);

    /// Homebrew installed packages cache (30 seconds) - fast refresh
    pub const HOMEBREW_INSTALLED: Duration = Duration::from_secs(30);

    /// Homebrew formula cache TTL (7 days) - formulas change slowly
    pub const HOMEBREW_FORMULAS: Duration = Duration::from_secs(7 * 24 * 60 * 60);

    /// Returns memory cache TTL in seconds (for APIs requiring u64)
    #[must_use]
    #[inline]
    pub const fn memory_secs() -> u64 {
        30 * 60
    }

    /// Returns disk cache TTL in seconds
    #[must_use]
    #[inline]
    pub const fn disk_secs() -> u64 {
        24 * 60 * 60
    }

    /// Returns mirror list cache TTL in seconds
    #[must_use]
    #[inline]
    pub const fn mirror_list_secs() -> u64 {
        6 * 60 * 60
    }
}

/// Network operation constants
pub mod network {
    use super::Duration;

    /// Maximum retries for transient network failures
    pub const MAX_RETRIES: u32 = 3;

    /// Initial backoff delay for retries
    pub const INITIAL_BACKOFF: Duration = Duration::from_millis(100);

    /// Mirror race timeout - how long to wait for fastest mirror
    pub const MIRROR_RACE_TIMEOUT: Duration = Duration::from_millis(2000);

    /// Debian package download timeout
    pub const DEBIAN_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
}

/// Daemon operation constants
pub mod daemon {
    use super::Duration;

    /// Request processing timeout
    pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

    /// TUI poll interval
    pub const TUI_POLL_INTERVAL: Duration = Duration::from_millis(100);
}

/// Session and analytics constants
pub mod session {
    use super::Duration;

    /// Session inactivity timeout (30 minutes)
    pub const TIMEOUT: Duration = Duration::from_secs(30 * 60);

    /// Returns session timeout in seconds (for APIs requiring i64)
    #[must_use]
    #[inline]
    pub const fn timeout_secs() -> i64 {
        1800
    }
}

/// Queue and buffer limits
pub mod limits {
    /// Maximum events in analytics queue before oldest are drained
    pub const MAX_EVENT_QUEUE_SIZE: usize = 1000;

    /// Number of events to drain when queue exceeds max size
    pub const EVENT_DRAIN_COUNT: usize = 500;

    /// Maximum search query length
    pub const MAX_SEARCH_QUERY_LEN: usize = 100;

    /// AUR RPC maximum URI length
    pub const AUR_RPC_MAX_URI: usize = 4400;

    /// AUR RPC info base URL length (pre-computed)
    pub const AUR_RPC_INFO_BASE_LEN: usize = 47;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_ttl_consistency() {
        // Verify the const functions match the Duration values
        assert_eq!(cache_ttl::MEMORY.as_secs(), cache_ttl::memory_secs());
        assert_eq!(cache_ttl::DISK.as_secs(), cache_ttl::disk_secs());
        assert_eq!(
            cache_ttl::MIRROR_LIST.as_secs(),
            cache_ttl::mirror_list_secs()
        );
    }

    #[test]
    fn test_session_timeout_consistency() {
        assert_eq!(session::TIMEOUT.as_secs() as i64, session::timeout_secs());
    }

    #[test]
    fn test_reasonable_timeouts() {
        // Ensure timeouts are within reasonable bounds
        assert!(http::DEFAULT_TIMEOUT.as_secs() >= 5);
        assert!(http::DEFAULT_TIMEOUT.as_secs() <= 60);
        assert!(http::DOWNLOAD_TIMEOUT.as_secs() >= 60);
        assert!(http::DOWNLOAD_TIMEOUT.as_secs() <= 600);
    }
}
