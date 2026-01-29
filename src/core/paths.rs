//! Shared filesystem paths with test-friendly overrides.

use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Default, Debug)]
struct PathOverrides {
    pacman_root: Option<PathBuf>,
    pacman_db_dir: Option<PathBuf>,
}

static OVERRIDES: OnceLock<RwLock<PathOverrides>> = OnceLock::new();

fn get_overrides() -> &'static RwLock<PathOverrides> {
    OVERRIDES.get_or_init(|| RwLock::new(PathOverrides::default()))
}

/// Set path overrides for testing. Safe and thread-safe.
pub fn set_test_overrides(root: Option<PathBuf>, db_dir: Option<PathBuf>) {
    let mut guard = get_overrides().write();
    guard.pacman_root = root;
    guard.pacman_db_dir = db_dir;
}

/// Reset all path overrides.
pub fn reset_test_overrides() {
    let mut guard = get_overrides().write();
    *guard = PathOverrides::default();
}

fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

fn fallback_home_dir() -> PathBuf {
    home::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Data directory (default: XDG data dir/omg or ~/.omg).
#[must_use]
pub fn data_dir() -> PathBuf {
    env_path("OMG_DATA_DIR").unwrap_or_else(|| {
        dirs::data_dir().map_or_else(|| fallback_home_dir().join(".omg"), |d| d.join("omg"))
    })
}

/// Daemon data directory (default: /var/lib/omg).
#[must_use]
pub fn daemon_data_dir() -> PathBuf {
    env_path("OMG_DAEMON_DATA_DIR").unwrap_or_else(|| {
        dirs::data_dir().map_or_else(|| PathBuf::from("/var/lib/omg"), |d| d.join("omg"))
    })
}

/// Config directory (default: XDG config dir/omg or ~/.config/omg).
#[must_use]
pub fn config_dir() -> PathBuf {
    env_path("OMG_CONFIG_DIR").unwrap_or_else(|| {
        dirs::config_dir().map_or_else(
            || fallback_home_dir().join(".config/omg"),
            |d| d.join("omg"),
        )
    })
}

/// Cache directory (default: XDG cache dir/omg or ~/.cache/omg).
/// When running with sudo, uses the original user's cache directory.
#[must_use]
pub fn cache_dir() -> PathBuf {
    env_path("OMG_CACHE_DIR").unwrap_or_else(|| {
        // If running as root via sudo, use original user's cache directory
        if let Ok(sudo_user) = std::env::var("SUDO_USER")
            && crate::core::is_root()
        {
            // Try SUDO_HOME first, fallback to /home/<username>
            let home = std::env::var("SUDO_HOME")
                .ok()
                .map_or_else(
                    || PathBuf::from(format!("/home/{sudo_user}")),
                    PathBuf::from,
                );
            
            return home.join(".cache/omg");
        }
        
        // Check DOAS_USER as well
        if let Ok(doas_user) = std::env::var("DOAS_USER")
            && crate::core::is_root()
        {
            let home = PathBuf::from(format!("/home/{doas_user}"));
            return home.join(".cache/omg");
        }

        // Normal case: use current user's cache directory
        dirs::cache_dir().map_or_else(|| fallback_home_dir().join(".cache/omg"), |d| d.join("omg"))
    })
}

/// Pacman root directory (default: /).
#[must_use]
pub fn pacman_root() -> PathBuf {
    let guard = get_overrides().read();
    if let Some(ref root) = guard.pacman_root {
        return root.clone();
    }
    env_path("OMG_PACMAN_ROOT").unwrap_or_else(|| PathBuf::from("/"))
}

/// Pacman database directory (default: /var/lib/pacman).
#[must_use]
pub fn pacman_db_dir() -> PathBuf {
    let guard = get_overrides().read();
    if let Some(ref db) = guard.pacman_db_dir {
        return db.clone();
    }
    env_path("OMG_PACMAN_DB_DIR").unwrap_or_else(|| pacman_root().join("var/lib/pacman"))
}

/// Pacman sync database directory (default: /var/lib/pacman/sync).
#[must_use]
pub fn pacman_sync_dir() -> PathBuf {
    env_path("OMG_PACMAN_SYNC_DIR").unwrap_or_else(|| pacman_db_dir().join("sync"))
}

/// Pacman local database directory (default: /var/lib/pacman/local).
#[must_use]
pub fn pacman_local_dir() -> PathBuf {
    env_path("OMG_PACMAN_LOCAL_DIR").unwrap_or_else(|| pacman_db_dir().join("local"))
}

/// Pacman package cache directory (default: /var/cache/pacman/pkg).
#[must_use]
pub fn pacman_cache_dir() -> PathBuf {
    env_path("OMG_PACMAN_CACHE_DIR").unwrap_or_else(|| pacman_root().join("var/cache/pacman/pkg"))
}

/// Pacman cache root directory (default: /var/cache/pacman).
#[must_use]
pub fn pacman_cache_root_dir() -> PathBuf {
    env_path("OMG_PACMAN_CACHE_ROOT_DIR").unwrap_or_else(|| pacman_root().join("var/cache/pacman"))
}

/// Pacman mirrorlist path (default: /etc/pacman.d/mirrorlist).
#[must_use]
pub fn pacman_mirrorlist_path() -> PathBuf {
    env_path("OMG_PACMAN_MIRRORLIST").unwrap_or_else(|| PathBuf::from("/etc/pacman.d/mirrorlist"))
}

/// Pacman configuration file path (default: /etc/pacman.conf).
#[must_use]
pub fn pacman_conf_path() -> PathBuf {
    env_path("OMG_PACMAN_CONF").unwrap_or_else(|| PathBuf::from("/etc/pacman.conf"))
}

/// Daemon socket path (default: $`XDG_RUNTIME_DIR/omg.sock`, /run/user/<uid>/omg.sock, or /tmp/omg.sock).
#[must_use]
pub fn socket_path() -> PathBuf {
    env_path("OMG_SOCKET_PATH").unwrap_or_else(|| {
        // 1. Try XDG_RUNTIME_DIR
        if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
            return PathBuf::from(runtime_dir).join("omg.sock");
        }

        // 2. Try common system location (/run/user/<uid>) which is often missing from env under sudo
        #[cfg(unix)]
        {
            let uid = rustix::process::getuid().as_raw();
            let system_run = PathBuf::from(format!("/run/user/{uid}/omg.sock"));
            if system_run.exists() {
                return system_run;
            }
        }

        // 3. Fallback to /tmp
        PathBuf::from("/tmp/omg.sock")
    })
}

/// Fast status file path for zero-IPC reads (daemon writes, CLI reads directly).
/// Located next to socket for same permissions/lifecycle.
#[must_use]
pub fn fast_status_path() -> PathBuf {
    // Derive from socket path to ensure same directory
    let sock = socket_path();
    sock.with_file_name("omg.status")
}

/// Install marker file path (tracks first run for telemetry).
#[must_use]
pub fn installed_marker_path() -> PathBuf {
    data_dir().join(".installed")
}

/// Returns true if running in hermetic test mode.
#[must_use]
pub fn test_mode() -> bool {
    matches!(
        std::env::var("OMG_TEST_MODE").as_deref(),
        Ok("1" | "true" | "TRUE")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_dir_returns_path() {
        let path = data_dir();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_config_dir_returns_path() {
        let path = config_dir();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_cache_dir_returns_path() {
        let path = cache_dir();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn test_socket_path_returns_path() {
        let path = socket_path();
        assert!(path.to_string_lossy().contains("omg.sock"));
    }

    #[test]
    fn test_fast_status_path_derives_from_socket() {
        let status = fast_status_path();
        assert!(status.to_string_lossy().contains("omg.status"));
    }

    #[test]
    fn test_installed_marker_in_data_dir() {
        let marker = installed_marker_path();
        assert!(marker.to_string_lossy().contains(".installed"));
    }

    #[test]
    fn test_pacman_root_default() {
        let root = pacman_root();
        assert!(root.to_string_lossy().starts_with('/'));
    }

    #[test]
    fn test_pacman_db_dir_under_root() {
        let db = pacman_db_dir();
        assert!(db.to_string_lossy().contains("pacman"));
    }

    #[test]
    fn test_pacman_sync_dir_under_db() {
        let sync = pacman_sync_dir();
        assert!(sync.to_string_lossy().contains("sync"));
    }

    #[test]
    fn test_pacman_local_dir_under_db() {
        let local = pacman_local_dir();
        assert!(local.to_string_lossy().contains("local"));
    }

    #[test]
    fn test_pacman_cache_dir() {
        let cache = pacman_cache_dir();
        assert!(cache.to_string_lossy().contains("cache"));
    }
}
