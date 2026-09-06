//! Shared filesystem paths.
//!
//! Pacman root/db paths honor in-process test overrides, but only in test and
//! debug builds: the override machinery is compiled out of release binaries so
//! production path resolution cannot be redirected at runtime.

use std::path::{Path, PathBuf};
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
    std::env::var_os(var)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[inline]
fn fallback_home_dir() -> PathBuf {
    // Never make persistent application state depend on the caller's current
    // directory. `/var/empty` is intentionally non-user-writable on Unix, so
    // an environment with no resolvable home fails explicitly instead of
    // scattering relative `.omg` directories through arbitrary projects.
    home::home_dir().unwrap_or_else(|| PathBuf::from("/var/empty"))
}

fn elevated_user_home() -> Option<PathBuf> {
    if !crate::core::is_root() {
        return None;
    }
    elevated_home_from(
        std::env::var("SUDO_USER").ok().as_deref(),
        std::env::var("SUDO_HOME").ok().as_deref(),
        std::env::var("DOAS_USER").ok().as_deref(),
    )
}

fn elevated_home_from(
    sudo_user: Option<&str>,
    _sudo_home: Option<&str>,
    doas_user: Option<&str>,
) -> Option<PathBuf> {
    elevated_home_from_lookup(sudo_user, doas_user, |user| {
        nix::unistd::User::from_name(user)
            .ok()
            .flatten()
            .map(|entry| entry.dir)
    })
}

fn elevated_home_from_lookup(
    sudo_user: Option<&str>,
    doas_user: Option<&str>,
    lookup: impl FnOnce(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let user = sudo_user
        .filter(|user| is_valid_username(user))
        .or_else(|| doas_user.filter(|user| is_valid_username(user)))?;
    lookup(user)
}

#[cfg(target_os = "macos")]
fn default_data_dir_for_home(home: &Path) -> PathBuf {
    home.join("Library/Application Support/omg")
}

#[cfg(not(target_os = "macos"))]
fn default_data_dir_for_home(home: &Path) -> PathBuf {
    home.join(".local/share/omg")
}

#[cfg(target_os = "macos")]
fn default_cache_dir_for_home(home: &Path) -> PathBuf {
    home.join("Library/Caches/omg")
}

#[cfg(not(target_os = "macos"))]
fn default_cache_dir_for_home(home: &Path) -> PathBuf {
    home.join(".cache/omg")
}

/// Data directory (default: XDG data dir/omg or ~/.omg).
/// Elevated child processes keep using the invoking user's state directory.
#[must_use]
pub fn data_dir() -> PathBuf {
    env_path("OMG_DATA_DIR").unwrap_or_else(|| {
        elevated_user_home().map_or_else(
            || dirs::data_dir().map_or_else(|| fallback_home_dir().join(".omg"), |d| d.join("omg")),
            |home| default_data_dir_for_home(&home),
        )
    })
}

/// Resolve a binary shipped next to the running executable.
///
/// Returns `None` when no sibling file exists, so callers fall back to
/// PATH lookup. One helper so every launcher agrees on existence
/// semantics (`is_file`).
#[must_use]
pub fn sibling_binary(name: &str) -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .filter(|path| path.is_file())
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
        elevated_user_home().map_or_else(
            || {
                dirs::config_dir().map_or_else(
                    || fallback_home_dir().join(".config/omg"),
                    |d| d.join("omg"),
                )
            },
            |home| home.join(".config/omg"),
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
        elevated_user_home().map_or_else(
            || {
                dirs::cache_dir()
                    .map_or_else(|| fallback_home_dir().join(".cache/omg"), |d| d.join("omg"))
            },
            |home| default_cache_dir_for_home(&home),
        )
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
///
/// Arch-only: the sole caller (`pacman_cache_dirs_result`) is arch-gated.
#[cfg(feature = "arch")]
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

#[cfg(feature = "arch")]
fn require_absolute_pacman_path(path: PathBuf, setting: &str) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        path.is_absolute(),
        "{setting} must be an absolute path: {}",
        path.display()
    );
    Ok(path)
}

/// Resolve pacman's root using test override, explicit environment, then
/// pacman.conf precedence. Unlike [`pacman_root`], configuration errors are
/// surfaced instead of silently selecting the host root.
#[cfg(feature = "arch")]
pub fn pacman_root_result() -> anyhow::Result<PathBuf> {
    #[cfg(any(test, debug_assertions))]
    if let Some(root) = overridden_pacman_root() {
        return require_absolute_pacman_path(root, "pacman root override");
    }
    if let Some(root) = env_path("OMG_PACMAN_ROOT").filter(|path| !path.as_os_str().is_empty()) {
        return require_absolute_pacman_path(root, "OMG_PACMAN_ROOT");
    }
    let config = crate::core::pacman_conf::PacmanConfig::parse(pacman_conf_path())?;
    require_absolute_pacman_path(
        config
            .root_dir
            .map_or_else(|| PathBuf::from("/"), PathBuf::from),
        "RootDir",
    )
}

fn pacman_env_dir(name: &str, require_root_owned: bool) -> Option<PathBuf> {
    let path = env_path(name).filter(|path| !path.as_os_str().is_empty())?;
    #[cfg(unix)]
    if require_root_owned {
        use std::os::unix::fs::MetadataExt as _;
        match std::fs::symlink_metadata(&path) {
            Ok(metadata)
                if metadata.file_type().is_dir()
                    && metadata.uid() == 0
                    && metadata.mode() & 0o022 == 0 =>
            {
                return Some(path);
            }
            _ => {
                tracing::warn!(
                    "Ignoring {name} {}: privileged pacman directories must be real, root-owned directories that are not group/world-writable",
                    path.display()
                );
                return None;
            }
        }
    }
    Some(path)
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
    pacman_env_dir("OMG_PACMAN_DB_DIR", crate::core::privilege::is_root())
        .unwrap_or_else(|| pacman_root().join("var/lib/pacman"))
}

/// Resolve pacman's database path using test override, explicit environment,
/// then pacman.conf. A configured RootDir relocates the default DBPath exactly
/// as pacman's `setdefaults` does.
#[cfg(feature = "arch")]
pub fn pacman_db_dir_result() -> anyhow::Result<PathBuf> {
    #[cfg(any(test, debug_assertions))]
    {
        let guard = get_overrides()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(ref db) = guard.pacman_db_dir {
            return require_absolute_pacman_path(db.clone(), "pacman database override");
        }
    }
    if let Some(db) = pacman_env_dir("OMG_PACMAN_DB_DIR", crate::core::privilege::is_root()) {
        return require_absolute_pacman_path(db, "OMG_PACMAN_DB_DIR");
    }

    let config = crate::core::pacman_conf::PacmanConfig::parse(pacman_conf_path())?;
    if let Some(db) = config.db_path {
        return require_absolute_pacman_path(PathBuf::from(db), "DBPath");
    }
    let root = if let Some(root) =
        env_path("OMG_PACMAN_ROOT").filter(|path| !path.as_os_str().is_empty())
    {
        require_absolute_pacman_path(root, "OMG_PACMAN_ROOT")?
    } else {
        require_absolute_pacman_path(
            config
                .root_dir
                .map_or_else(|| PathBuf::from("/"), PathBuf::from),
            "RootDir",
        )?
    };
    Ok(root.join("var/lib/pacman"))
}

/// Pacman sync database directory (default: /var/lib/pacman/sync).
#[cfg(test)]
#[must_use]
pub fn pacman_sync_dir() -> PathBuf {
    pacman_env_dir("OMG_PACMAN_SYNC_DIR", crate::core::privilege::is_root())
        .unwrap_or_else(|| pacman_db_dir().join("sync"))
}

#[cfg(feature = "arch")]
pub fn pacman_sync_dir_result() -> anyhow::Result<PathBuf> {
    if let Some(sync) = pacman_env_dir("OMG_PACMAN_SYNC_DIR", crate::core::privilege::is_root()) {
        return require_absolute_pacman_path(sync, "OMG_PACMAN_SYNC_DIR");
    }
    Ok(pacman_db_dir_result()?.join("sync"))
}

/// Pacman local database directory (default: /var/lib/pacman/local).
#[must_use]
pub fn pacman_local_dir() -> PathBuf {
    pacman_env_dir("OMG_PACMAN_LOCAL_DIR", crate::core::privilege::is_root())
        .unwrap_or_else(|| pacman_db_dir().join("local"))
}

#[cfg(feature = "arch")]
pub fn pacman_local_dir_result() -> anyhow::Result<PathBuf> {
    if let Some(local) = pacman_env_dir("OMG_PACMAN_LOCAL_DIR", crate::core::privilege::is_root()) {
        return require_absolute_pacman_path(local, "OMG_PACMAN_LOCAL_DIR");
    }
    Ok(pacman_db_dir_result()?.join("local"))
}

#[cfg(feature = "arch")]
pub fn pacman_cache_dirs_result() -> anyhow::Result<Vec<PathBuf>> {
    if let Some(cache_dir) = pacman_env_dir("OMG_PACMAN_CACHE_DIR", true) {
        return Ok(vec![require_absolute_pacman_path(
            cache_dir,
            "OMG_PACMAN_CACHE_DIR",
        )?]);
    }

    let root = pacman_root_result()?;
    if !pacman_root_overridden() {
        let config = crate::core::pacman_conf::PacmanConfig::parse(pacman_conf_path())?;
        if !config.cache_dirs.is_empty() {
            return Ok(config
                .cache_dirs
                .into_iter()
                .map(PathBuf::from)
                .map(|path| {
                    if path.is_absolute() {
                        path
                    } else {
                        root.join(path)
                    }
                })
                .collect());
        }
    }
    Ok(vec![root.join("var/cache/pacman/pkg")])
}

/// Pacman cache root directory (default: /var/cache/pacman).
#[must_use]
pub fn pacman_cache_root_dir() -> PathBuf {
    env_path("OMG_PACMAN_CACHE_ROOT_DIR").unwrap_or_else(|| pacman_root().join("var/cache/pacman"))
}

#[cfg(feature = "arch")]
pub fn pacman_cache_root_dir_result() -> anyhow::Result<PathBuf> {
    if let Some(root) = env_path("OMG_PACMAN_CACHE_ROOT_DIR") {
        return require_absolute_pacman_path(root, "OMG_PACMAN_CACHE_ROOT_DIR");
    }
    Ok(pacman_root_result()?.join("var/cache/pacman"))
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

fn test_mode_value(value: Option<&str>, debug_assertions: bool) -> bool {
    debug_assertions
        && value.is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

/// Returns true if a debug/test binary is running in hermetic test mode.
///
/// Release binaries ignore `OMG_TEST_MODE`: an inherited environment variable
/// must never replace real package/runtime state with synthetic fixtures.
#[must_use]
pub fn test_mode() -> bool {
    test_mode_value(
        std::env::var("OMG_TEST_MODE").ok().as_deref(),
        cfg!(debug_assertions),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_requires_debug_build_and_explicit_truthy_value() {
        assert!(test_mode_value(Some("1"), true));
        assert!(test_mode_value(Some("TRUE"), true));
        assert!(!test_mode_value(Some("1"), false));
        assert!(!test_mode_value(Some("0"), true));
        assert!(!test_mode_value(Some(""), true));
        assert!(!test_mode_value(None, true));
    }

    #[test]
    fn elevated_home_uses_the_account_database() {
        let lookup = |user: &str| Some(PathBuf::from(format!("/var/home/{user}")));
        assert_eq!(
            elevated_home_from_lookup(Some("alice"), None, lookup),
            Some(PathBuf::from("/var/home/alice"))
        );
        assert_eq!(
            elevated_home_from_lookup(None, Some("bob"), lookup),
            Some(PathBuf::from("/var/home/bob"))
        );
        assert_eq!(
            elevated_home_from_lookup(Some("../root"), None, lookup),
            None
        );
        assert_eq!(
            elevated_home_from_lookup(Some("missing"), None, |_| None),
            None
        );
    }

    #[test]
    #[serial_test::serial]
    fn empty_path_override_is_treated_as_unset() {
        temp_env::with_var("OMG_DATA_DIR", Some(""), || {
            assert!(!data_dir().as_os_str().is_empty());
        });
        temp_env::with_var("OMG_CACHE_DIR", Some(""), || {
            assert!(!cache_dir().as_os_str().is_empty());
        });
        temp_env::with_var("OMG_SOCKET_PATH", Some(""), || {
            assert!(socket_path().ends_with("omg.sock"));
        });
    }

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

    #[cfg(feature = "arch")]
    #[test]
    #[serial_test::serial]
    fn pacman_paths_honor_rootdir_and_dbpath_from_configuration() {
        let directory = tempfile::tempdir().expect("temporary config directory");
        let config = directory.path().join("pacman.conf");
        std::fs::write(&config, "[options]\nRootDir = /srv/arch-root\n")
            .expect("write pacman config");

        temp_env::with_vars(
            [
                ("OMG_PACMAN_CONF", Some(config.as_os_str())),
                ("OMG_PACMAN_ROOT", None::<&std::ffi::OsStr>),
                ("OMG_PACMAN_DB_DIR", None::<&std::ffi::OsStr>),
            ],
            || {
                assert_eq!(
                    pacman_root_result().unwrap(),
                    PathBuf::from("/srv/arch-root")
                );
                assert_eq!(
                    pacman_db_dir_result().unwrap(),
                    PathBuf::from("/srv/arch-root/var/lib/pacman")
                );
            },
        );

        std::fs::write(
            &config,
            "[options]\nRootDir = /srv/arch-root\nDBPath = /srv/pacman-db\n",
        )
        .expect("rewrite pacman config");
        temp_env::with_vars(
            [
                ("OMG_PACMAN_CONF", Some(config.as_os_str())),
                ("OMG_PACMAN_ROOT", None::<&std::ffi::OsStr>),
                ("OMG_PACMAN_DB_DIR", None::<&std::ffi::OsStr>),
            ],
            || {
                assert_eq!(
                    pacman_db_dir_result().unwrap(),
                    PathBuf::from("/srv/pacman-db")
                );
            },
        );
    }

    #[cfg(unix)]
    #[test]
    fn privileged_pacman_override_rejects_writable_and_symlinked_directories() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let temp = tempfile::tempdir().expect("temporary pacman path");
        let writable = temp.path().join("writable");
        std::fs::create_dir(&writable).expect("create writable directory");
        std::fs::set_permissions(&writable, std::fs::Permissions::from_mode(0o777))
            .expect("set writable mode");
        temp_env::with_var("OMG_PACMAN_DB_DIR", Some(writable.as_os_str()), || {
            assert!(pacman_env_dir("OMG_PACMAN_DB_DIR", true).is_none());
        });

        let trusted_target = temp.path().join("target");
        std::fs::create_dir(&trusted_target).expect("create target directory");
        std::fs::set_permissions(&trusted_target, std::fs::Permissions::from_mode(0o700))
            .expect("set target mode");
        let linked = temp.path().join("linked");
        symlink(&trusted_target, &linked).expect("link override");
        temp_env::with_var("OMG_PACMAN_DB_DIR", Some(linked.as_os_str()), || {
            assert!(pacman_env_dir("OMG_PACMAN_DB_DIR", true).is_none());
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
