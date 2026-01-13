//! OMG Settings and Configuration

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
}

impl Default for Settings {
    fn default() -> Self {
        let data_dir = directories::ProjectDirs::from("com", "omg", "omg")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| {
                home::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".omg")
            });

        // Socket in XDG_RUNTIME_DIR or /tmp
        let socket_path = std::env::var("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"))
            .join("omg.sock");

        Settings {
            shims_enabled: false, // PATH modification is default (faster)
            data_dir,
            socket_path,
            default_shell: "zsh".to_string(),
            auto_update: false,
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
            Ok(Settings::default())
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
        let config_dir = directories::ProjectDirs::from("com", "omg", "omg")
            .map(|d| d.config_dir().to_path_buf())
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;

        Ok(config_dir.join("config.toml"))
    }

    /// Get the versions directory
    pub fn versions_dir(&self) -> PathBuf {
        self.data_dir.join("versions")
    }

    /// Get the shims directory
    pub fn shims_dir(&self) -> PathBuf {
        self.data_dir.join("shims")
    }
}
