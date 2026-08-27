//! Common types used throughout OMG

use crate::package_managers::types::Version;
use serde::{Deserialize, Serialize};

/// Package source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackageSource {
    /// Official Arch Linux repositories
    Official,
    /// Arch User Repository
    Aur,
}

impl PackageSource {
    /// Parse either the wire labels (`official`/`aur`) or human display labels.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        if label.eq_ignore_ascii_case("official") {
            Some(Self::Official)
        } else if label.eq_ignore_ascii_case("aur") {
            Some(Self::Aur)
        } else {
            None
        }
    }
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

#[cfg(test)]
mod tests {
    use super::PackageSource;

    #[test]
    fn package_source_accepts_wire_and_display_casing() {
        assert_eq!(
            PackageSource::from_label("official"),
            Some(PackageSource::Official)
        );
        assert_eq!(
            PackageSource::from_label("Official"),
            Some(PackageSource::Official)
        );
        assert_eq!(PackageSource::from_label("aur"), Some(PackageSource::Aur));
        assert_eq!(PackageSource::from_label("AUR"), Some(PackageSource::Aur));
        assert_eq!(PackageSource::from_label("unknown"), None);
    }
}
