//! Shared filesystem paths.
//!
//! Pacman root/db paths honor in-process test overrides, but only in test and
//! debug builds: the override machinery is compiled out of release binaries so
//! production path resolution cannot be redirected at runtime.

use std::path::PathBuf;
#[cfg(any(test, debug_assertions))]
use std::sync::OnceLock;
#[cfg(any(test, debug_assertions))]
use std::sync::RwLock;

#[cfg(any(test, debug_assertions))]
#[derive(Default, Debug)]
struct PathOverrides {
    pacman_root: Option<PathBuf>,
    pacman_db_dir: Option<PathBuf>,
}

#[cfg(any(test, debug_assertions))]
static OVERRIDES: OnceLock<RwLock<PathOverrides>> = OnceLock::new();

#[cfg(any(test, debug_assertions))]
#[inline]
fn get_overrides() -> &'static RwLock<PathOverrides> {
    OVERRIDES.get_or_init(|| RwLock::new(PathOverrides::default()))
}

/// Set pacman path overrides for testing. Safe and thread-safe.
///
/// Only available in test and debug builds (`cfg(any(test, debug_assertions))`);
/// release binaries ship without this API.
#[cfg(any(test, debug_assertions))]
pub fn set_test_overrides(root: Option<PathBuf>, db_dir: Option<PathBuf>) {
    let mut guard = get_overrides()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.pacman_root = root;
    guard.pacman_db_dir = db_dir;
}

/// Reset all path overrides. Test/debug builds only; see [`set_test_overrides`].
#[cfg(any(test, debug_assertions))]
pub fn reset_test_overrides() {
    let mut guard = get_overrides()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = PathOverrides::default();
}

#[inline]
fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

#[inline]
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

/// Daemon data directory (default: XDG data dir/omg, falling back to
/// `/var/lib/omg` when no XDG data directory can be resolved).
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

#[inline]
fn is_valid_username(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\0')
        && !name.contains("..")
        && name.len() <= 256
}

/// Cache directory (default: XDG cache dir/omg or ~/.cache/omg).
/// When running with sudo, uses the original user's cache directory.
#[must_use]
pub fn cache_dir() -> PathBuf {
    env_path("OMG_CACHE_DIR").unwrap_or_else(|| {
        if let Ok(sudo_user) = std::env::var("SUDO_USER")
            && crate::core::is_root()
            && is_valid_username(&sudo_user)
        {
            // SUDO_HOME is environment-controlled and evaluated as root:
            // apply the same charset/length rules as SUDO_USER before it
            // becomes a path prefix.
            // SUDO_HOME is an absolute HOME path (e.g. /var/home/alice on
            // Silverblue), not a username — the old username validation
            // rejected every legitimate value containing '/'.
            let home = match std::env::var("SUDO_HOME") {
                Ok(dir)
                    if std::path::Path::new(&dir).is_absolute()
                        && !dir.contains('\0')
                        && !dir.contains("..") =>
                {
                    PathBuf::from(dir)
                }
                Ok(dir) => {
                    tracing::warn!(
                        "Ignoring unsafe SUDO_HOME {dir:?}; falling back to /home/{sudo_user}"
                    );
                    PathBuf::from(format!("/home/{sudo_user}"))
                }
                Err(_) => PathBuf::from(format!("/home/{sudo_user}")),
            };

            return home.join(".cache/omg");
        }

        if let Ok(doas_user) = std::env::var("DOAS_USER")
            && crate::core::is_root()
            && is_valid_username(&doas_user)
        {
            let home = PathBuf::from(format!("/home/{doas_user}"));
            return home.join(".cache/omg");
        }

        dirs::cache_dir().map_or_else(|| fallback_home_dir().join(".cache/omg"), |d| d.join("omg"))
    })
}

#[cfg(any(test, debug_assertions))]
fn overridden_pacman_root() -> Option<PathBuf> {
    let guard = get_overrides()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.pacman_root.clone()
}

/// Whether the pacman root was redirected via a test override or an explicit,
/// non-empty `OMG_PACMAN_ROOT`. When set, pacman.conf `CacheDir` resolution is
/// skipped so harness runs stay self-contained.
fn pacman_root_overridden() -> bool {
    #[cfg(any(test, debug_assertions))]
    if overridden_pacman_root().is_some() {
        return true;
    }
    std::env::var_os("OMG_PACMAN_ROOT").is_some_and(|value| !value.is_empty())
}

/// Pacman root directory (default: /). Honors [`set_test_overrides`] in test
/// and debug builds only.
#[must_use]
pub fn pacman_root() -> PathBuf {
    #[cfg(any(test, debug_assertions))]
    if let Some(root) = overridden_pacman_root() {
        return root;
    }
    env_path("OMG_PACMAN_ROOT")
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// Pacman database directory (default: /var/lib/pacman). Honors
/// [`set_test_overrides`] in test and debug builds only.
#[must_use]
pub fn pacman_db_dir() -> PathBuf {
    #[cfg(any(test, debug_assertions))]
    {
        let guard = get_overrides()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(ref db) = guard.pacman_db_dir {
            return db.clone();
        }
    }
    env_path("OMG_PACMAN_DB_DIR")
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| pacman_root().join("var/lib/pacman"))
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

/// Pacman package cache directories in configured priority order.
#[must_use]
pub fn pacman_cache_dirs() -> Vec<PathBuf> {
    if let Some(cache_dir) =
        env_path("OMG_PACMAN_CACHE_DIR").filter(|path| !path.as_os_str().is_empty())
    {
        return vec![cache_dir];
    }

    if !pacman_root_overridden()
        && let Some(cache_dirs) = configured_pacman_cache_dirs()
    {
        return cache_dirs;
    }

    vec![pacman_root().join("var/cache/pacman/pkg")]
}

#[cfg(feature = "arch")]
fn configured_pacman_cache_dirs() -> Option<Vec<PathBuf>> {
    let config = crate::core::pacman_conf::PacmanConfig::parse(pacman_conf_path()).ok()?;
    if config.cache_dirs.is_empty() {
        return None;
    }

    let root = pacman_root();
    Some(
        config
            .cache_dirs
            .into_iter()
            .map(|configured| {
                let path = PathBuf::from(configured);
                if path.is_absolute() {
                    path
                } else {
                    root.join(path)
                }
            })
            .collect(),
    )
}

#[cfg(not(feature = "arch"))]
fn configured_pacman_cache_dirs() -> Option<Vec<PathBuf>> {
    None
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

/// Daemon socket path.
///
/// Falls back to a UID-specific private directory instead of the shared `/tmp`
/// namespace. The daemon must call [`prepare_socket_parent`] before binding;
/// clients call [`validate_socket_parent`] before connecting.
#[must_use]
pub fn socket_path() -> PathBuf {
    env_path("OMG_SOCKET_PATH").unwrap_or_else(|| {
        if let Some(runtime_dir) = env_path("XDG_RUNTIME_DIR")
            && !runtime_dir.as_os_str().is_empty()
        {
            runtime_dir.join("omg.sock")
        } else {
            platform_socket_path()
        }
    })
}

#[cfg(unix)]
fn platform_socket_path() -> PathBuf {
    let uid = rustix::process::getuid().as_raw();
    let system_runtime_dir = PathBuf::from(format!("/run/user/{uid}"));
    if system_runtime_dir.is_dir() {
        system_runtime_dir.join("omg.sock")
    } else {
        uid_temp_socket_path(uid)
    }
}

#[cfg(unix)]
fn uid_temp_socket_path(uid: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/omg-{uid}/omg.sock"))
}

#[cfg(not(unix))]
fn platform_socket_path() -> PathBuf {
    PathBuf::from("omg.sock")
}

/// Validate that the parent directory of `socket_path` is safe to connect to.
///
/// The directory must be a real directory (not a symlink), owned by the current
/// uid, and not group/world accessible. Clients MUST call this before
/// connecting; daemons must have called [`prepare_socket_parent`] before binding.
///
/// # Errors
/// Returns `PermissionDenied` for symlinked, foreign-owned, or group/world
/// accessible parents, and `InvalidInput` when the socket path has no parent.
#[cfg(unix)]
pub fn validate_socket_parent(socket_path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let parent = socket_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "daemon socket path must have a parent directory",
        )
    })?;
    let metadata = std::fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "daemon runtime path is not a real directory: {}",
                parent.display()
            ),
        ));
    }

    let expected_uid = rustix::process::getuid().as_raw();
    if metadata.uid() != expected_uid {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("daemon runtime directory is not owned by uid {expected_uid}"),
        ));
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!(
                "daemon runtime directory must not be group/world accessible: {}",
                parent.display()
            ),
        ));
    }
    Ok(())
}

/// Create the parent directory of `socket_path` (mode `0700`) if missing and
/// validate it with [`validate_socket_parent`]. The daemon MUST call this
/// before binding the socket.
///
/// # Errors
/// Propagates I/O errors from directory creation and permission setup, plus
/// every [`validate_socket_parent`] failure condition.
#[cfg(unix)]
pub fn prepare_socket_parent(socket_path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    let parent = socket_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "daemon socket path must have a parent directory",
        )
    })?;
    if !parent.exists() {
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700).create(parent)?;
    }
    validate_socket_parent(socket_path)
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
    fn data_dir_is_non_empty() {
        let path = data_dir();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn config_dir_is_non_empty() {
        let path = config_dir();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn cache_dir_is_non_empty() {
        let path = cache_dir();
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn socket_path_names_the_socket_file() {
        temp_env::with_var_unset("OMG_SOCKET_PATH", || {
            let path = socket_path();
            assert!(path.to_string_lossy().contains("omg.sock"));
        });
    }

    #[cfg(unix)]
    #[test]
    fn temp_socket_fallback_is_scoped_to_the_uid() {
        assert_eq!(
            uid_temp_socket_path(42),
            PathBuf::from("/tmp/omg-42/omg.sock")
        );
    }

    #[cfg(unix)]
    #[test]
    fn socket_parent_is_created_private_and_validated() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temp dir");
        let socket = directory.path().join("runtime/omg.sock");

        prepare_socket_parent(&socket).expect("prepare socket parent");
        validate_socket_parent(&socket).expect("validate socket parent");

        let mode = std::fs::metadata(socket.parent().expect("socket parent"))
            .expect("parent metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o700);
    }

    #[cfg(unix)]
    #[test]
    fn group_accessible_socket_parent_is_rejected() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temp dir");
        let parent = directory.path().join("runtime");
        std::fs::create_dir(&parent).expect("create runtime dir");
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o750))
            .expect("set insecure mode");

        let error = validate_socket_parent(&parent.join("omg.sock"))
            .expect_err("group-accessible runtime dir must be rejected");
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn fast_status_path_sits_next_to_the_socket() {
        temp_env::with_var_unset("OMG_SOCKET_PATH", || {
            let status = fast_status_path();
            assert!(status.to_string_lossy().contains("omg.status"));
        });
    }

    #[test]
    fn installed_marker_lives_in_the_data_dir() {
        let marker = installed_marker_path();
        assert!(marker.to_string_lossy().contains(".installed"));
    }

    #[test]
    fn pacman_root_default_is_absolute() {
        temp_env::with_var_unset("OMG_PACMAN_ROOT", || {
            let root = pacman_root();
            assert_eq!(root, PathBuf::from("/"));
        });
    }

    #[test]
    fn pacman_db_dir_defaults_under_the_root() {
        temp_env::with_var_unset("OMG_PACMAN_DB_DIR", || {
            let db = pacman_db_dir();
            assert_eq!(db, PathBuf::from("/var/lib/pacman"));
        });
    }

    #[test]
    fn pacman_sync_dir_defaults_under_the_db_dir() {
        temp_env::with_var_unset("OMG_PACMAN_SYNC_DIR", || {
            let sync = pacman_sync_dir();
            assert_eq!(sync, pacman_db_dir().join("sync"));
        });
    }

    #[test]
    fn pacman_local_dir_defaults_under_the_db_dir() {
        temp_env::with_var_unset("OMG_PACMAN_LOCAL_DIR", || {
            let local = pacman_local_dir();
            assert_eq!(local, pacman_db_dir().join("local"));
        });
    }
}
