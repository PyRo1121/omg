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

impl std::str::FromStr for RuntimeBackend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "native" => Ok(Self::Native),
            "mise" => Ok(Self::Mise),
            "native-then-mise" | "native_then_mise" | "native_then-mise" => {
                Ok(Self::NativeThenMise)
            }
            _ => Err(format!(
                "Unknown runtime backend: {s} (expected native, mise, native-then-mise)"
            )),
        }
    }
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
