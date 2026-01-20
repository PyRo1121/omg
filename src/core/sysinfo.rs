//! System information detection for build optimization
//!
//! Detects hardware capabilities and available build tools to configure
//! optimal build settings during init wizard.

use std::path::Path;
use std::process::Command;

/// System hardware information
#[derive(Debug, Clone)]
pub struct SystemInfo {
    /// Number of logical CPU cores
    pub cpu_cores: usize,
    /// Total RAM in gigabytes
    pub ram_gb: f64,
    /// Whether ccache is installed
    pub ccache_available: bool,
    /// Whether sccache is installed
    pub sccache_available: bool,
    /// Whether distcc is installed
    pub distcc_available: bool,
}

/// Build configuration recommendations based on hardware
#[derive(Debug, Clone)]
pub struct BuildRecommendation {
    /// Recommended MAKEFLAGS value
    pub makeflags: String,
    /// Whether to enable ccache
    pub enable_ccache: bool,
    /// Whether to enable sccache
    pub enable_sccache: bool,
    /// Whether to disable `secure_makepkg` for speed
    pub disable_secure_makepkg: bool,
    /// Recommended build concurrency
    pub build_concurrency: usize,
    /// Human-readable explanation of recommendations
    pub explanation: Vec<String>,
}

impl SystemInfo {
    /// Detect system hardware information
    #[must_use]
    pub fn detect() -> Self {
        Self {
            cpu_cores: detect_cpu_cores(),
            ram_gb: detect_ram_gb(),
            ccache_available: is_tool_available("ccache"),
            sccache_available: is_tool_available("sccache"),
            distcc_available: is_tool_available("distcc"),
        }
    }

    /// Generate build recommendations based on detected hardware
    #[must_use]
    pub fn recommend(&self) -> BuildRecommendation {
        let mut explanation = Vec::new();

        // MAKEFLAGS - use all cores, with load limit
        let makeflags = if self.cpu_cores > 1 {
            explanation.push(format!(
                "Using -j{} for parallel compilation ({}x speedup potential)",
                self.cpu_cores, self.cpu_cores
            ));
            format!("-j{}", self.cpu_cores)
        } else {
            String::new()
        };

        // ccache - great for C/C++ rebuilds
        let enable_ccache = self.ccache_available;
        if enable_ccache {
            explanation.push(
                "ccache detected → enabling compiler cache (50-90% faster rebuilds)".to_string(),
            );
        } else {
            explanation.push("Install 'ccache' for faster C/C++ rebuilds".to_string());
        }

        // sccache - great for Rust
        let enable_sccache = self.sccache_available && !enable_ccache;
        if self.sccache_available {
            if enable_sccache {
                explanation.push("sccache detected → enabling Rust compiler cache".to_string());
            } else {
                explanation.push(
                    "sccache available (using ccache instead for broader coverage)".to_string(),
                );
            }
        }

        // RAM considerations
        let disable_secure_makepkg = self.ram_gb >= 16.0;
        if self.ram_gb >= 16.0 {
            explanation.push(format!(
                "{:.0}GB RAM detected → disabling cleanbuild for faster rebuilds",
                self.ram_gb
            ));
        } else if self.ram_gb < 8.0 {
            explanation.push(format!(
                "{:.1}GB RAM detected → consider reducing parallel jobs for large packages",
                self.ram_gb
            ));
        }

        // Build concurrency for AUR operations
        let build_concurrency = if self.cpu_cores >= 8 {
            4.min(self.cpu_cores / 2)
        } else if self.cpu_cores >= 4 {
            2
        } else {
            1
        };

        if build_concurrency > 1 {
            explanation.push(format!(
                "Enabling {build_concurrency} concurrent AUR builds"
            ));
        }

        BuildRecommendation {
            makeflags,
            enable_ccache,
            enable_sccache,
            disable_secure_makepkg,
            build_concurrency,
            explanation,
        }
    }
}

/// Detect number of CPU cores
fn detect_cpu_cores() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(1)
}

/// Detect total RAM in gigabytes from /proc/meminfo
fn detect_ram_gb() -> f64 {
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                // Format: "MemTotal:       16384000 kB"
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(kb_str) = parts.get(1)
                    && let Ok(kb) = kb_str.parse::<u64>()
                {
                    return kb as f64 / 1_048_576.0; // kB to GB
                }
            }
        }
    }
    // Fallback: assume 8GB
    8.0
}

/// Check if a tool is available in PATH
fn is_tool_available(name: &str) -> bool {
    // Check PATH first
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            if Path::new(dir).join(name).exists() {
                return true;
            }
        }
    }

    // Fallback to which command
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cpu_cores() {
        let cores = detect_cpu_cores();
        assert!(cores >= 1);
    }

    #[test]
    fn test_detect_ram_gb() {
        let ram = detect_ram_gb();
        assert!(ram > 0.0);
    }

    #[test]
    fn test_system_info_detect() {
        let info = SystemInfo::detect();
        assert!(info.cpu_cores >= 1);
        assert!(info.ram_gb > 0.0);
    }

    #[test]
    fn test_build_recommendation() {
        let info = SystemInfo {
            cpu_cores: 8,
            ram_gb: 16.0,
            ccache_available: true,
            sccache_available: false,
            distcc_available: false,
        };
        let rec = info.recommend();
        assert_eq!(rec.makeflags, "-j8");
        assert!(rec.enable_ccache);
        assert!(!rec.enable_sccache);
        assert!(rec.disable_secure_makepkg);
        assert_eq!(rec.build_concurrency, 4);
    }
}
