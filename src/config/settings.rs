//! OMG Settings and Configuration

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::core::{RuntimeBackend, paths};

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

    /// Runtime resolution backend (native, mise, native-then-mise)
    pub runtime_backend: RuntimeBackend,

    /// AUR build configuration
    pub aur: AurBuildSettings,
}

/// AUR build configuration
#[allow(clippy::struct_excessive_bools)]
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
            runtime_backend: RuntimeBackend::default(),
            aur: AurBuildSettings::default(),
        }
    }
}

impl Default for AurBuildSettings {
    fn default() -> Self {
        let jobs = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(1);
        Self {
            build_method: AurBuildMethod::Native,
            build_concurrency: jobs.max(1),
            review_pkgbuild: false,
            secure_makepkg: true,
            allow_unsafe_builds: true,
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
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    /// Save settings to config file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;

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
