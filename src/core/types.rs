//! Common types used throughout OMG

use crate::package_managers::types::Version;
use serde::{Deserialize, Serialize};

/// Runtime resolution backend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeBackend {
    Native,
    Mise,
    #[default]
    NativeThenMise,
}

/// Package source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageSource {
    /// Official Arch Linux repositories
    Official,
    /// Arch User Repository
    Aur,
}

/// Package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: Version,
    pub description: String,
    pub source: PackageSource,
    pub installed: bool,
}

impl std::fmt::Display for PackageSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Official => write!(f, "Official"),
            Self::Aur => write!(f, "AUR"),
        }
    }
}
