//! OMG Settings and Configuration

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::core::paths;

/// Maximum config file size (1MB) to prevent `DoS` via large configs
const MAX_CONFIG_SIZE: u64 = 1024 * 1024;

/// Maximum metadata cache TTL (7 days in seconds)
const MAX_CACHE_TTL_SECS: u64 = 7 * 24 * 60 * 60;

/// Validate a path doesn't contain path traversal sequences
fn validate_config_path(path: &Path, field_name: &str) -> Result<()> {
    let path_str = path.to_string_lossy();

    // Check for path traversal
    if path_str.contains("..") {
        anyhow::bail!("Config error: {field_name} contains path traversal sequence '..'");
    }

    // Check for null bytes
    if path_str.contains('\0') {
        anyhow::bail!("Config error: {field_name} contains null byte");
    }

    // Reject absolute paths outside of home/cache directories for safety
    if path.is_absolute() {
        let is_safe = path_str.starts_with("/home/")
            || path_str.starts_with("/tmp/")
            || path_str.starts_with("/var/cache/")
            || path_str.starts_with("/var/tmp/");

        if !is_safe {
            anyhow::bail!(
                "Config error: {field_name} absolute path must be under /home/, /tmp/, or /var/cache/"
            );
        }
    }

    Ok(())
}

/// OMG configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Enable shims (optional, default: false - use PATH modification)
    pub shims_enabled: bool,

    /// OMG data directory
    pub data_dir: PathBuf,

    /// Daemon socket path
    pub socket_path: PathBuf,

    /// Default shell for hooks
    pub default_shell: String,

    /// Auto-update runtime versions on install
    pub auto_update: bool,

    /// Whether telemetry is enabled
    pub telemetry_enabled: bool,

    /// AUR build configuration
    pub aur: AurBuildSettings,
}

/// AUR build configuration
#[expect(clippy::struct_excessive_bools)] // Configuration struct: booleans map to user-facing toggle options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AurBuildSettings {
    /// Build method for AUR packages
    pub build_method: AurBuildMethod,
    /// Maximum concurrent AUR builds
    pub build_concurrency: usize,
    /// Require interactive PKGBUILD review before building
    pub review_pkgbuild: bool,
    /// Use stricter makepkg flags (cleanbuild/verifysource)
    pub secure_makepkg: bool,
    /// Allow native builds without sandboxing
    pub allow_unsafe_builds: bool,
    /// Use AUR metadata archive for bulk update checks
    pub use_metadata_archive: bool,
    /// Metadata archive cache TTL (seconds)
    pub metadata_cache_ttl_secs: u64,
    /// Custom MAKEFLAGS (overrides auto -jN)
    pub makeflags: Option<String>,
    /// Custom PKGDEST (shared package cache)
    pub pkgdest: Option<PathBuf>,
    /// Custom SRCDEST (shared source cache)
    pub srcdest: Option<PathBuf>,
    /// Enable build cache re-use based on PKGBUILD hash
    pub cache_builds: bool,
    /// Enable ccache integration
    pub enable_ccache: bool,
    /// Optional ccache directory
    pub ccache_dir: Option<PathBuf>,
    /// Enable sccache integration
    pub enable_sccache: bool,
    /// Optional sccache directory
    pub sccache_dir: Option<PathBuf>,
}

/// AUR build method options
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AurBuildMethod {
    Bubblewrap,
    Chroot,
    Native,
}

impl Default for Settings {
    fn default() -> Self {
        let data_dir = paths::data_dir();

        // Socket in XDG_RUNTIME_DIR or /tmp
        let socket_path = paths::socket_path();

        Self {
            shims_enabled: false, // PATH modification is default (faster)
            data_dir,
            socket_path,
            default_shell: "zsh".to_string(),
            auto_update: false,
            telemetry_enabled: true,
            aur: AurBuildSettings::default(),
        }
    }
}

impl Default for AurBuildSettings {
    fn default() -> Self {
        let jobs = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);

        Self {
            build_method: AurBuildMethod::Bubblewrap,
            build_concurrency: jobs.max(1),
            review_pkgbuild: true,
            secure_makepkg: true,
            allow_unsafe_builds: false,
            use_metadata_archive: true,
            metadata_cache_ttl_secs: 300,
            makeflags: None,
            pkgdest: None,
            srcdest: None,
            cache_builds: true,
            enable_ccache: false,
            ccache_dir: None,
            enable_sccache: false,
            sccache_dir: None,
        }
    }
}

impl Settings {
    /// Load settings from config file
    /// Reject unknown top-level and `aur`-table keys before deserialization.
    ///
    /// Serde would otherwise silently ignore typos, fabricating a working-looking
    /// config from misspelled settings.
    fn validate_known_keys(content: &str) -> Result<()> {
        let table: toml::Table = toml::from_str(content).context("Config is not valid TOML")?;

        const ROOT_KEYS: [&str; 7] = [
            "shims_enabled",
            "data_dir",
            "socket_path",
            "default_shell",
            "auto_update",
            "telemetry_enabled",
            "aur",
        ];
        const AUR_KEYS: [&str; 15] = [
            "build_method",
            "build_concurrency",
            "review_pkgbuild",
            "secure_makepkg",
            "allow_unsafe_builds",
            "use_metadata_archive",
            "metadata_cache_ttl_secs",
            "makeflags",
            "pkgdest",
            "srcdest",
            "cache_builds",
            "enable_ccache",
            "ccache_dir",
            "enable_sccache",
            "sccache_dir",
        ];

        // Legacy sections written by older omg releases. They carry no
        // settings the current schema consumes; recognize them so existing
        // installs keep working, but tell the user they are ignored.
        const LEGACY_KEYS: [&str; 3] = ["cache", "general", "security"];

        let mut unknown = Vec::new();
        for key in table.keys() {
            if ROOT_KEYS.contains(&key.as_str()) {
                continue;
            }
            if LEGACY_KEYS.contains(&key.as_str()) {
                // Settings::load runs many times per invocation (telemetry
                // gates, completions, ...); warn once per process so a daily
                // command is not spammed with the same notice.
                use std::sync::atomic::{AtomicBool, Ordering};
                static WARNED: AtomicBool = AtomicBool::new(false);
                if !WARNED.swap(true, Ordering::Relaxed) {
                    tracing::warn!(
                        key = key.as_str(),
                        "config section '{key}' is deprecated and ignored by this omg version"
                    );
                }
                continue;
            }
            unknown.push(key.clone());
        }
        if let Some(aur) = table.get("aur").and_then(|value| value.as_table()) {
            for key in aur.keys() {
                if !AUR_KEYS.contains(&key.as_str()) {
                    unknown.push(format!("aur.{key}"));
                }
            }
        }

        if unknown.is_empty() {
            return Ok(());
        }
        anyhow::bail!(
            "unknown configuration keys: {}. Allowed top-level keys: {}; allowed 'aur' keys: {}",
            unknown.join(", "),
            ROOT_KEYS.join(", "),
            AUR_KEYS.join(", ")
        )
    }

    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            // Security: Check file size before reading to prevent DoS
            let metadata = std::fs::metadata(&config_path)
                .with_context(|| format!("Failed to stat config: {}", config_path.display()))?;
            if metadata.len() > MAX_CONFIG_SIZE {
                anyhow::bail!(
                    "Config file too large: {} bytes (max {} bytes)",
                    metadata.len(),
                    MAX_CONFIG_SIZE
                );
            }

            let content = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config: {}", config_path.display()))?;
            Self::validate_known_keys(&content).with_context(|| {
                format!(
                    "Invalid config: {} (unknown keys are rejected to catch typos)",
                    config_path.display()
                )
            })?;
            let settings: Self = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config: {}", config_path.display()))?;

            // Security: Validate all path fields to prevent path traversal
            settings.validate_paths()?;

            // Security: Validate TTL bounds
            if settings.aur.metadata_cache_ttl_secs > MAX_CACHE_TTL_SECS {
                anyhow::bail!(
                    "aur.metadata_cache_ttl_secs too large: {} (max {} = 7 days)",
                    settings.aur.metadata_cache_ttl_secs,
                    MAX_CACHE_TTL_SECS
                );
            }

            Ok(settings)
        } else {
            Ok(Self::default())
        }
    }

    /// Validate all path fields to prevent path traversal attacks
    fn validate_paths(&self) -> Result<()> {
        if let Some(ref path) = self.aur.pkgdest {
            validate_config_path(path, "aur.pkgdest")?;
        }
        if let Some(ref path) = self.aur.srcdest {
            validate_config_path(path, "aur.srcdest")?;
        }
        if let Some(ref path) = self.aur.ccache_dir {
            validate_config_path(path, "aur.ccache_dir")?;
        }
        if let Some(ref path) = self.aur.sccache_dir {
            validate_config_path(path, "aur.sccache_dir")?;
        }
        Ok(())
    }

    /// Save settings to config file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config dir: {}", parent.display()))?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(&config_path, &content)
            .with_context(|| format!("Failed to write config: {}", config_path.display()))?;

        Ok(())
    }

    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = paths::config_dir();

        Ok(config_dir.join("config.toml"))
    }

    /// Get the versions directory
    #[must_use]
    pub fn versions_dir(&self) -> PathBuf {
        self.data_dir.join("versions")
    }

    /// Get the shims directory
    #[must_use]
    pub fn shims_dir(&self) -> PathBuf {
        self.data_dir.join("shims")
    }
}
