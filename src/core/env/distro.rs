//! Distro detection helpers
//!
//! Supports detection of:
//! - Arch Linux and derivatives (Manjaro, `EndeavourOS`, etc.)
//! - Debian and derivatives (Ubuntu, Linux Mint, `Pop!_OS`, etc.)
//! - Fedora and derivatives (RHEL, `CentOS`, Rocky, Alma, etc.)
//! - macOS (via `uname`)
//!
//! Windows Subsystem for Linux is detected through its Linux distribution.
//! Native Windows is not supported.

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
#[must_use]
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
                _ => Distro::Unknown,
            };
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

            classify(id, id_like)
        }

        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Distro::Unknown
        }
    })
}

/// Returns true if running on Arch Linux or derivatives
#[must_use]
pub fn is_arch_like() -> bool {
    matches!(detect_distro(), Distro::Arch)
}

/// Returns true if running on Debian or Ubuntu
#[must_use]
pub fn is_debian_like() -> bool {
    matches!(detect_distro(), Distro::Debian | Distro::Ubuntu)
}

/// Returns true if running on Fedora or RHEL-family (`CentOS`, Rocky, Alma)
#[must_use]
pub fn is_fedora_like() -> bool {
    matches!(detect_distro(), Distro::Fedora)
}

/// Returns true if running on macOS
#[must_use]
pub fn is_macos() -> bool {
    matches!(detect_distro(), Distro::MacOS)
}

/// Check if we should use Debian backend based on current distro and features
#[must_use]
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

/// Classify a distribution from its os-release `ID`/`ID_LIKE` values.
///
/// Order matters:
/// 1. Arch family (pacman/ALPM backend).
/// 2. Fedora/RHEL family (DNF backend).
/// 3. Ubuntu family — including derivatives such as Pop!_OS, KDE neon, and
///    elementary that declare `ubuntu` in `ID_LIKE`. They must classify as
///    [`Distro::Ubuntu`] so consumers like self-update pick Ubuntu-built
///    artifacts; package-manager selection treats Ubuntu and Debian alike.
/// 4. Debian family (apt backend). Pure-Debian-claimed derivatives such as
///    Linux Mint (`ID_LIKE="debian"`) land here; they still get the apt
///    backend.
fn classify(id: &str, id_like: &str) -> Distro {
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

    // Ubuntu and Ubuntu-family derivatives (checked before debian since every
    // Ubuntu derivative is also debian-like)
    if id == "ubuntu" || is_like(id, id_like, "ubuntu") {
        return Distro::Ubuntu;
    }

    // Debian and other derivatives
    if id == "debian" || is_like(id, id_like, "debian") {
        return Distro::Debian;
    }

    Distro::Unknown
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arch_family_including_derivatives() {
        assert_eq!(classify("arch", ""), Distro::Arch);
        assert_eq!(classify("manjaro", "arch"), Distro::Arch);
        assert_eq!(classify("endeavouros", "arch"), Distro::Arch);
        assert_eq!(classify("archarm", "arch"), Distro::Arch);
        assert_eq!(classify("cachyos", "arch"), Distro::Arch);
    }

    #[test]
    fn fedora_family() {
        assert_eq!(classify("fedora", ""), Distro::Fedora);
        assert_eq!(classify("rhel", "fedora"), Distro::Fedora);
        assert_eq!(classify("centos", "rhel fedora"), Distro::Fedora);
        assert_eq!(classify("rocky", "rhel fedora"), Distro::Fedora);
        assert_eq!(classify("ol", "fedora rhel centos"), Distro::Fedora);
        assert_eq!(classify("amzn", "centos rhel fedora"), Distro::Fedora);
    }

    #[test]
    fn ubuntu_and_ubuntu_family_derivatives_classify_as_ubuntu() {
        assert_eq!(classify("ubuntu", "debian"), Distro::Ubuntu);
        // Pop!_OS declares ID_LIKE="ubuntu debian"
        assert_eq!(classify("pop", "ubuntu debian"), Distro::Ubuntu);
        // KDE neon
        assert_eq!(classify("neon", "ubuntu debian"), Distro::Ubuntu);
        // elementary OS
        assert_eq!(classify("elementary", "ubuntu debian"), Distro::Ubuntu);
        // Trisquel
        assert_eq!(classify("trisquel", "ubuntu"), Distro::Ubuntu);
    }

    #[test]
    fn debian_family() {
        assert_eq!(classify("debian", ""), Distro::Debian);
        // Linux Mint claims only "debian" in ID_LIKE; it still gets the apt
        // backend, so Debian classification is acceptable there.
        assert_eq!(classify("linuxmint", "debian"), Distro::Debian);
        assert_eq!(classify("kali", "debian"), Distro::Debian);
    }

    #[test]
    fn unknown_distributions_do_not_match_any_family() {
        assert_eq!(classify("alpine", ""), Distro::Unknown);
        assert_eq!(classify("opensuse-leap", "suse opensuse"), Distro::Unknown);
        assert_eq!(classify("", ""), Distro::Unknown);
    }
}
