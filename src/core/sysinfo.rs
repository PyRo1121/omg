//! System information detection for build optimization
//!
//! Detects hardware capabilities and available build tools to configure
//! optimal build settings during init wizard.

use anyhow::{Context, Result};
use std::num::NonZero;

/// System hardware information
#[derive(Debug, Clone)]
pub struct SystemInfo {
    /// Number of logical CPU cores
    pub cpu_cores: usize,
    /// Total RAM in gigabytes
    pub ram_gb: f64,
    /// Kernel version
    pub kernel: String,
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
    pub fn detect() -> Result<Self> {
        Ok(Self {
            cpu_cores: detect_cpu_cores()?,
            ram_gb: detect_ram_gb()?,
            kernel: detect_kernel()?,
            ccache_available: is_tool_available("ccache"),
            sccache_available: is_tool_available("sccache"),
            distcc_available: is_tool_available("distcc"),
        })
    }

    /// Generate build recommendations based on detected hardware
    #[must_use]
    pub fn recommend(&self) -> BuildRecommendation {
        let mut explanation = Vec::new();

        // MAKEFLAGS - use all detected logical cores.
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
                "{:.0}GB RAM detected → disabling secure clean builds for faster rebuilds",
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
fn detect_cpu_cores() -> Result<usize> {
    std::thread::available_parallelism()
        .map(NonZero::get)
        .context("Failed to detect CPU parallelism")
}

/// Detect kernel version
fn detect_kernel() -> Result<String> {
    let content =
        std::fs::read_to_string("/proc/version").context("Failed to read /proc/version")?;
    content
        .split_whitespace()
        .nth(2)
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Kernel version missing from /proc/version"))
}

/// Detect total RAM in gigabytes from /proc/meminfo
fn detect_ram_gb() -> Result<f64> {
    let content =
        std::fs::read_to_string("/proc/meminfo").context("Failed to read /proc/meminfo")?;
    ram_gb_from_meminfo(&content)
}

fn ram_gb_from_meminfo(content: &str) -> Result<f64> {
    for line in content.lines() {
        let Some(rest) = line.strip_prefix("MemTotal:") else {
            continue;
        };
        let kb_str = rest
            .split_whitespace()
            .next()
            .ok_or_else(|| anyhow::anyhow!("MemTotal line has no size"))?;
        let kb: u64 = kb_str
            .parse()
            .with_context(|| format!("Invalid MemTotal value {kb_str}"))?;
        return Ok(kb as f64 / 1_048_576.0);
    }
    anyhow::bail!("MemTotal not found in /proc/meminfo")
}

/// Check whether an executable tool can be resolved from `PATH`.
fn is_tool_available(name: &str) -> bool {
    which::which(name).is_ok()
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_cpu_cores() {
        let cores = detect_cpu_cores().unwrap();
        assert!(cores >= 1);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_detect_ram_gb() {
        let ram = detect_ram_gb().unwrap();
        assert!(ram > 0.0);
    }

    #[test]
    fn ram_gb_from_meminfo_parses_total() {
        let ram = ram_gb_from_meminfo("MemTotal:       16384000 kB\n").unwrap();
        assert!((ram - 15.625).abs() < 0.001);
    }

    #[test]
    fn ram_gb_from_meminfo_rejects_missing_total() {
        let error = ram_gb_from_meminfo("MemFree: 1 kB\n").unwrap_err();
        assert!(error.to_string().contains("MemTotal not found"));
    }

    #[test]
    fn ram_gb_from_meminfo_rejects_invalid_size() {
        let error = ram_gb_from_meminfo("MemTotal: not-a-number kB\n").unwrap_err();
        assert!(error.to_string().contains("Invalid MemTotal value"));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_system_info_detect() {
        let info = SystemInfo::detect().unwrap();
        assert!(info.cpu_cores >= 1);
        assert!(info.ram_gb > 0.0);
        assert!(!info.kernel.is_empty());
        assert_ne!(info.kernel, "unknown");
    }

    #[test]
    fn test_build_recommendation() {
        let info = SystemInfo {
            cpu_cores: 8,
            ram_gb: 16.0,
            kernel: "6.12.0".to_string(),
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
