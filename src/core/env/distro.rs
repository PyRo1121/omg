//! Distro detection helpers

use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distro {
    Arch,
    Debian,
    Ubuntu,
    Unknown,
}

pub fn detect_distro() -> Distro {
    let data = fs::read_to_string("/etc/os-release").ok();
    let map = data.as_deref().map(parse_os_release).unwrap_or_default();

    let id = map.get("ID").map(String::as_str).unwrap_or_default();
    let id_like = map.get("ID_LIKE").map(String::as_str).unwrap_or_default();

    if is_like(id, id_like, "arch") {
        return Distro::Arch;
    }

    if id == "ubuntu" {
        return Distro::Ubuntu;
    }

    if id == "debian" || is_like(id, id_like, "debian") {
        return Distro::Debian;
    }

    Distro::Unknown
}

pub fn is_arch_like() -> bool {
    matches!(detect_distro(), Distro::Arch)
}

pub fn is_debian_like() -> bool {
    matches!(detect_distro(), Distro::Debian | Distro::Ubuntu)
}

/// Check if we should use Debian backend based on current distro and features
pub fn use_debian_backend() -> bool {
    #[cfg(feature = "debian")]
    {
        return is_debian_like();
    }

    #[cfg(not(feature = "debian"))]
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
