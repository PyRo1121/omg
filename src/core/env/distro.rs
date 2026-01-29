//! Distro detection helpers
//!
//! Supports detection of:
//! - Arch Linux and derivatives (Manjaro, `EndeavourOS`, etc.)
//! - Debian and derivatives (Ubuntu, Linux Mint, `Pop!_OS`, etc.)
//! - Fedora and derivatives (RHEL, `CentOS`, Rocky, Alma, etc.)
//! - macOS (via `uname`)
//! - Windows (via `cfg`)

use std::collections::HashMap;
use std::fs;
use std::sync::OnceLock;

/// Supported operating systems and distributions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distro {
    /// Arch Linux and derivatives (`pacman`/ALPM)
    Arch,
    /// Debian `GNU/Linux`
    Debian,
    /// Ubuntu
    Ubuntu,
    /// Fedora, RHEL, `CentOS`, Rocky, Alma (`DNF`/RPM)
    Fedora,
    /// macOS (`Homebrew`)
    MacOS,
    /// Windows (`Scoop`/`Winget`)
    Windows,
    /// Unknown or unsupported
    Unknown,
}

impl Distro {
    /// Returns the package manager name for this distro
    #[must_use]
    pub const fn package_manager_name(&self) -> &'static str {
        match self {
            Self::Arch => "pacman",
            Self::Debian | Self::Ubuntu => "apt",
            Self::Fedora => "dnf",
            Self::MacOS => "brew",
            Self::Windows => "scoop",
            Self::Unknown => "unknown",
        }
    }

    /// Returns true if this is a Linux distribution
    #[must_use]
    pub const fn is_linux(&self) -> bool {
        matches!(
            self,
            Self::Arch | Self::Debian | Self::Ubuntu | Self::Fedora
        )
    }
}

/// Detect the current operating system/distribution
pub fn detect_distro() -> Distro {
    static DISTRO: OnceLock<Distro> = OnceLock::new();
    *DISTRO.get_or_init(|| {
        // Test mode override
        if crate::core::paths::test_mode()
            && let Ok(overridden) = std::env::var("OMG_TEST_DISTRO")
        {
            return match overridden.to_lowercase().as_str() {
                "arch" => Distro::Arch,
                "debian" => Distro::Debian,
                "ubuntu" => Distro::Ubuntu,
                "fedora" | "rhel" | "centos" | "rocky" | "alma" => Distro::Fedora,
                "macos" | "darwin" => Distro::MacOS,
                "windows" => Distro::Windows,
                _ => Distro::Unknown,
            };
        }

        // Windows detection (compile-time)
        #[cfg(target_os = "windows")]
        {
            return Distro::Windows;
        }

        // macOS detection (compile-time)
        #[cfg(target_os = "macos")]
        {
            return Distro::MacOS;
        }

        // Linux detection via /etc/os-release
        #[cfg(target_os = "linux")]
        {
            let data = fs::read_to_string("/etc/os-release").ok();
            let map = data.as_deref().map(parse_os_release).unwrap_or_default();

            let id = map.get("ID").map(String::as_str).unwrap_or_default();
            let id_like = map.get("ID_LIKE").map(String::as_str).unwrap_or_default();

            // Arch Linux and derivatives
            if is_like(id, id_like, "arch") {
                return Distro::Arch;
            }

            // Fedora and RHEL-family
            if id == "fedora"
                || is_like(id, id_like, "fedora")
                || is_like(id, id_like, "rhel")
                || id == "rhel"
                || id == "centos"
                || id == "rocky"
                || id == "almalinux"
            {
                return Distro::Fedora;
            }

            // Ubuntu (check before debian since ubuntu is debian-like)
            if id == "ubuntu" {
                return Distro::Ubuntu;
            }

            // Debian and other derivatives
            if id == "debian" || is_like(id, id_like, "debian") {
                return Distro::Debian;
            }

            Distro::Unknown
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Distro::Unknown
        }
    })
}

/// Returns true if running on Arch Linux or derivatives
pub fn is_arch_like() -> bool {
    matches!(detect_distro(), Distro::Arch)
}

/// Returns true if running on Debian or Ubuntu
pub fn is_debian_like() -> bool {
    matches!(detect_distro(), Distro::Debian | Distro::Ubuntu)
}

/// Returns true if running on Fedora or RHEL-family (`CentOS`, Rocky, Alma)
pub fn is_fedora_like() -> bool {
    matches!(detect_distro(), Distro::Fedora)
}

/// Returns true if running on macOS
pub fn is_macos() -> bool {
    matches!(detect_distro(), Distro::MacOS)
}

/// Returns true if running on Windows
pub fn is_windows() -> bool {
    matches!(detect_distro(), Distro::Windows)
}

/// Check if we should use Debian backend based on current distro and features
pub fn use_debian_backend() -> bool {
    #[cfg(feature = "debian")]
    {
        is_debian_like()
    }

    #[cfg(not(feature = "debian"))]
    {
        false
    }
}

/// Check if we should use Fedora/DNF backend
pub fn use_fedora_backend() -> bool {
    #[cfg(feature = "fedora")]
    {
        is_fedora_like()
    }

    #[cfg(not(feature = "fedora"))]
    {
        false
    }
}

/// Check if we should use Homebrew backend
pub fn use_homebrew_backend() -> bool {
    #[cfg(feature = "macos")]
    {
        is_macos()
    }

    #[cfg(not(feature = "macos"))]
    {
        false
    }
}

/// Check if we should use Windows backend
pub fn use_windows_backend() -> bool {
    #[cfg(feature = "windows")]
    {
        is_windows()
    }

    #[cfg(not(feature = "windows"))]
    {
        false
    }
}

fn is_like(id: &str, id_like: &str, needle: &str) -> bool {
    id == needle
        || id_like
            .split_whitespace()
            .any(|value| value.trim() == needle)
}

fn parse_os_release(contents: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let cleaned = value.trim().trim_matches('"');
            map.insert(key.to_string(), cleaned.to_string());
        }
    }
    map
}
