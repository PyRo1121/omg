//! Native Python runtime manager - PURE RUST
//!
//! Downloads pre-built Python binaries from python-build-standalone.
//!
//! Features:
//! - Pre-built binaries (no compilation required)
//! - Automatic version detection
//! - Virtual environment support

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::common::{
    GithubRelease, activate_version_with_linked_binary, begin_staged_install,
    complete_staged_install, download_with_progress, extract_tar_gz, fetch_github_releases,
    normalize_version, parse_sha256_digest, print_already_installed, print_installed, print_using,
    remove_file_best_effort, validate_download_filename, version_cmp,
};
use crate::{cli::style, core::http::download_client};

const PBS_RELEASES_URL: &str =
    "https://api.github.com/repos/indygreg/python-build-standalone/releases";
const PBS_LIST_PER_PAGE: u32 = 10;
const PBS_LIST_MAX_PAGES: u32 = 1;
const PBS_INSTALL_PER_PAGE: u32 = 10;
const PBS_INSTALL_MAX_PAGES: u32 = 20;

/// Python version info for available versions
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PythonVersion {
    pub version: String,
    pub prerelease: bool,
}

pub(crate) struct PythonManager {
    versions_dir: PathBuf,
    client: &'static reqwest::Client,
}

impl PythonManager {
    pub fn new() -> Self {
        Self {
            versions_dir: super::DATA_DIR.join("versions/python"),
            client: download_client(),
        }
    }

    /// List available Python versions from python-build-standalone
    pub async fn list_available(&self) -> Result<Vec<PythonVersion>> {
        if crate::core::paths::test_mode() {
            return Ok(vec![
                PythonVersion {
                    version: "3.12.0".to_string(),
                    prerelease: false,
                },
                PythonVersion {
                    version: "3.11.0".to_string(),
                    prerelease: false,
                },
            ]);
        }
        let target = python_target()?;
        let releases = fetch_github_releases(
            self.client,
            PBS_RELEASES_URL,
            PBS_LIST_PER_PAGE,
            PBS_LIST_MAX_PAGES,
            |_| false,
        )
        .await
        .context("Failed to fetch Python releases from GitHub")?;

        Ok(Self::parse_python_versions(&releases, &target))
    }

    /// Build the newest-first version list from standard gzip assets for the
    /// host target. Duplicate versions across release pages collapse to one
    /// entry.
    fn parse_python_versions(releases: &[GithubRelease], target: &str) -> Vec<PythonVersion> {
        let suffix = format!("{target}-install_only.tar.gz");
        let mut seen = std::collections::HashSet::new();
        for release in releases {
            for asset in &release.assets {
                let Some(version) = Self::parse_cpython_version(&asset.name) else {
                    continue;
                };
                if asset.name.ends_with(&suffix) {
                    seen.insert(version);
                }
            }
        }

        let mut result: Vec<PythonVersion> = seen
            .into_iter()
            .map(|version| PythonVersion {
                prerelease: Self::parse_python_version(&version)
                    .is_some_and(|(_, prerelease)| prerelease.is_some()),
                version,
            })
            .collect();
        result.sort_by(|a, b| Self::python_version_cmp(&b.version, &a.version));
        result
    }

    /// Parse a PBS asset version only from the text after `cpython-` and
    /// before the required build-stamp `+`, e.g. `3.14.7` or `3.15.0rc2`.
    fn parse_cpython_version(asset_name: &str) -> Option<String> {
        let (_, tail) = asset_name.split_once("cpython-")?;
        let (raw, _) = tail.split_once('+')?;
        Self::is_python_version(raw).then(|| raw.to_owned())
    }

    fn is_python_version(raw: &str) -> bool {
        Self::parse_python_version(raw).is_some()
    }

    /// Split a raw CPython version into its numeric base and prerelease suffix.
    /// The prerelease rank is `a`=0, `b`=1, `rc`=2. Freethreaded `t` tags and
    /// malformed suffixes are rejected.
    fn parse_python_version(raw: &str) -> Option<(&str, Option<(u8, u32)>)> {
        let (base, suffix) = raw.split_at(
            raw.find(|c: char| c.is_ascii_alphabetic())
                .unwrap_or(raw.len()),
        );
        let prerelease = if suffix.is_empty() {
            None
        } else {
            let (rank, number) = if let Some(rest) = suffix.strip_prefix("rc") {
                (2u8, rest)
            } else if let Some(rest) = suffix.strip_prefix('b') {
                (1u8, rest)
            } else if let Some(rest) = suffix.strip_prefix('a') {
                (0u8, rest)
            } else {
                return None;
            };
            Some((rank, number.parse::<u32>().ok()?))
        };
        let mut parts = base.split('.');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(major), Some(minor), Some(patch), None)
                if [major, minor, patch]
                    .iter()
                    .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())) => {}
            _ => return None,
        }
        Some((base, prerelease))
    }

    /// Order CPython versions by numeric precedence with `a` < `b` < `rc`
    /// and every prerelease below its own stable version (`3.15.0rc2` <
    /// `3.15.0`). Unparsable inputs fall back to the shared version order.
    fn python_version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (Self::parse_python_version(a), Self::parse_python_version(b)) {
            (Some((base_a, pre_a)), Some((base_b, pre_b))) => {
                let base = Self::numeric_component_cmp(base_a, base_b);
                if base != Ordering::Equal {
                    return base;
                }
                match (pre_a, pre_b) {
                    (None, None) => Ordering::Equal,
                    (None, Some(_)) => Ordering::Greater,
                    (Some(_), None) => Ordering::Less,
                    (Some((rank_a, serial_a)), Some((rank_b, serial_b))) => {
                        rank_a.cmp(&rank_b).then(serial_a.cmp(&serial_b))
                    }
                }
            }
            _ => version_cmp(a, b),
        }
    }

    fn numeric_component_cmp(a: &str, b: &str) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        let mut left = a.split('.');
        let mut right = b.split('.');
        loop {
            match (left.next(), right.next()) {
                (None, None) => return Ordering::Equal,
                (l, r) => {
                    let l = l.unwrap_or("0").parse::<u64>().unwrap_or(0);
                    let r = r.unwrap_or("0").parse::<u64>().unwrap_or(0);
                    if l != r {
                        return l.cmp(&r);
                    }
                }
            }
        }
    }

    /// Match only the exact standard gzip suffix `{target}-install_only.tar.gz`
    /// so freethreaded, stripped, and alternate-compression variants never win.
    fn asset_matches_version(name: &str, version: &str, target: &str) -> bool {
        Self::parse_cpython_version(name).as_deref() == Some(version)
            && name.ends_with(&format!("{target}-install_only.tar.gz"))
    }

    /// Install Python - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        let version = self.resolve_requested_version(&version).await?;
        crate::core::security::validate_runtime_version(&version)?;
        let version_dir = self.versions_dir.join(&version);

        if crate::runtimes::common::is_valid_version_dir(&version_dir) {
            print_already_installed("Python", &version);
            return self.use_version(&version);
        }

        if crate::core::paths::test_mode() {
            fs::create_dir_all(version_dir.join("bin"))?;
            fs::write(version_dir.join("bin/python3.12"), "mock")?;
            #[cfg(unix)]
            std::os::unix::fs::symlink("python3.12", version_dir.join("bin/python3"))?;
            #[cfg(not(unix))]
            fs::copy(
                version_dir.join("bin/python3.12"),
                version_dir.join("bin/python3"),
            )?;
            fs::write(
                version_dir.join(super::common::TEST_RUNTIME_MARKER),
                "debug-only synthetic runtime\n",
            )?;
            println!(
                "{} OMG_TEST_MODE active — synthetic Python runtime was not activated",
                style::caution("⚠")
            );
            print_installed("Python", &version);
            return Ok(());
        }

        println!(
            "{} Installing Python {}...\n",
            style::runtime("OMG"),
            style::caution(&version)
        );

        let target = python_target()?;

        println!(
            "{} Finding Python {} release...",
            style::informative("→"),
            version
        );

        let releases = fetch_github_releases(
            self.client,
            PBS_RELEASES_URL,
            PBS_INSTALL_PER_PAGE,
            PBS_INSTALL_MAX_PAGES,
            |release| {
                release
                    .assets
                    .iter()
                    .any(|asset| Self::asset_matches_version(&asset.name, &version, &target))
            },
        )
        .await
        .context("Failed to fetch Python releases")?;

        let asset = releases
            .iter()
            .flat_map(|release| &release.assets)
            .find(|asset| Self::asset_matches_version(&asset.name, &version, &target))
            .ok_or_else(|| {
                anyhow::anyhow!("Python {version} not found. Try: omg list python --available")
            })?;

        let url = asset
            .browser_download_url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Python release asset has no browser download URL"))?;
        let asset_name = validate_download_filename(&asset.name)?;
        let checksum = asset
            .digest
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Python release asset has no SHA-256 digest"))
            .and_then(|digest| parse_sha256_digest(digest, "GitHub Python release"))?;

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading {}...", style::informative("→"), asset_name);
        let download_path = self.versions_dir.join(asset_name);
        download_with_progress(self.client, url, &download_path, &checksum).await?;

        println!("{} Extracting (pure Rust)...", style::informative("→"));
        let staging = begin_staged_install(&self.versions_dir)?;
        extract_tar_gz(&download_path, staging.path(), 1).await?;
        complete_staged_install(&staging, &version_dir, &version)?;

        remove_file_best_effort(&download_path, "runtime archive");

        print_installed("Python", &version);
        self.use_version(&version)?;

        Ok(())
    }

    /// Resolve a partial version request (`3`, `3.12`) to the newest matching
    /// stable python-build-standalone version before any download URL is
    /// built; `asset_matches_version` requires exact string equality, so an
    /// unresolved partial would never match an asset. Prerelease entries are
    /// ignored, exact and non-numeric requests pass through unchanged, and
    /// exact prerelease requests may install.
    async fn resolve_requested_version(&self, version: &str) -> Result<String> {
        if !crate::runtimes::common::is_partial_version(version) {
            return Ok(version.to_owned());
        }
        let available = self.list_available().await?;
        let names: Vec<String> = available
            .into_iter()
            .filter(|entry| !entry.prerelease)
            .map(|entry| entry.version)
            .collect();
        Ok(crate::runtimes::resolve_version_request(&names, version))
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        activate_version_with_linked_binary(
            &self.versions_dir,
            &version,
            Path::new("bin/python3"),
        )?;
        print_using("Python", &version, &self.versions_dir.join("current/bin"));
        Ok(())
    }

    /// Remove an installed version. Refuses the active version.
    pub fn uninstall(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        super::common::uninstall_version(&self.versions_dir, &version)
    }
}

// Generate common runtime manager methods (list_installed, current_version)
crate::runtimes::common::impl_runtime_common!(PythonManager);

fn python_target() -> Result<String> {
    let arch = match std::env::consts::ARCH {
        "x86_64" | "aarch64" => std::env::consts::ARCH,
        arch => anyhow::bail!("Unsupported architecture for Python: {arch}"),
    };
    match std::env::consts::OS {
        "linux" => Ok(format!("{arch}-unknown-linux-gnu")),
        "macos" => Ok(format!("{arch}-apple-darwin")),
        other => anyhow::bail!("Unsupported operating system for Python: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn install_discovery_uses_small_pages_without_shortening_history() -> Result<()> {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}/releases", listener.local_addr()?);
        let client = reqwest::Client::builder().no_proxy().build()?;
        let server = async {
            let mut requests = 0;
            loop {
                let (mut stream, _) = listener.accept().await?;
                let request = {
                    let mut reader = BufReader::new(&mut stream).take(8192);
                    let mut first = String::new();
                    reader.read_line(&mut first).await?;
                    loop {
                        let mut line = String::new();
                        anyhow::ensure!(
                            reader.read_line(&mut line).await? > 0,
                            "Incomplete fixture request"
                        );
                        if line == "\r\n" {
                            break;
                        }
                    }
                    first
                };
                let target = request
                    .split_whitespace()
                    .nth(1)
                    .context("Missing request target")?;
                let parsed = reqwest::Url::parse(&format!("http://localhost{target}"))?;
                let query: std::collections::HashMap<_, _> =
                    parsed.query_pairs().into_owned().collect();
                let size: usize = query
                    .get("per_page")
                    .context("Missing page size")?
                    .parse()?;
                let page: usize = query.get("page").context("Missing page number")?.parse()?;
                requests += 1;
                if size > 10 {
                    stream.write_all(b"HTTP/1.1 504 Gateway Timeout\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
                    return Ok::<_, anyhow::Error>(requests);
                }
                anyhow::ensure!(
                    size > 0 && page > 0 && page <= 200,
                    "Invalid pagination request"
                );
                let end = (page * size).min(200);
                let releases: Vec<_> = ((page - 1) * size..end).map(|index| {
                    let assets = if index == 199 {
                        vec![serde_json::json!({"name": "cpython-3.12.14+20260901-x86_64-unknown-linux-gnu-install_only.tar.gz"})]
                    } else { Vec::new() };
                    serde_json::json!({"tag_name": index.to_string(), "assets": assets})
                }).collect();
                let body = serde_json::to_vec(&releases)?;
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await?;
                stream.write_all(&body).await?;
                if end == 200 {
                    return Ok(requests);
                }
            }
        };
        let discovery = fetch_github_releases(
            &client,
            &url,
            PBS_INSTALL_PER_PAGE,
            PBS_INSTALL_MAX_PAGES,
            |release| {
                release.assets.iter().any(|asset| {
                    PythonManager::asset_matches_version(
                        &asset.name,
                        "3.12.14",
                        "x86_64-unknown-linux-gnu",
                    )
                })
            },
        );
        let (fetched, served) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            tokio::join!(discovery, server)
        })
        .await
        .context("Release discovery did not finish within its fixture deadline")?;
        let releases = fetched.context("Bounded release discovery must succeed")?;
        assert_eq!(served?, 20);
        assert_eq!(releases.len(), 200);
        assert!(
            releases
                .last()
                .is_some_and(|release| release.tag_name == "199")
        );
        Ok(())
    }

    #[test]
    fn test_python_manager_new() {
        let mgr = PythonManager::new();
        assert!(mgr.versions_dir.ends_with("python"));
    }

    #[test]
    fn test_extract_cpython_version() {
        assert_eq!(
            PythonManager::parse_cpython_version(
                "cpython-3.12.0+20231002-x86_64-unknown-linux-gnu-install_only.tar.gz"
            ),
            Some("3.12.0".to_string())
        );
        assert_eq!(
            PythonManager::parse_cpython_version(
                "cpython-3.15.0rc2+20250708-x86_64-unknown-linux-gnu-install_only.tar.gz"
            ),
            Some("3.15.0rc2".to_string())
        );
        // A `+` build stamp is required.
        assert_eq!(
            PythonManager::parse_cpython_version("cpython-3.11.5-x86_64.tar.gz"),
            None
        );
    }

    #[test]
    fn rc_and_stable_prerelease_forms_parse() {
        assert!(PythonManager::is_python_version("3.14.7"));
        assert!(PythonManager::is_python_version("3.15.0rc2"));
        assert!(PythonManager::is_python_version("3.15.0rc10"));
        assert!(PythonManager::is_python_version("3.15.0b1"));
        assert!(PythonManager::is_python_version("3.15.0a4"));
        assert!(PythonManager::is_python_version("3.14.0rc1"));
    }

    #[test]
    fn malformed_and_variant_version_forms_are_rejected() {
        for malformed in [
            "3.14",
            "3",
            "",
            "3..7",
            ".3.14.7",
            "3.14.7.",
            "3.14.7+extra",
            "3.14.7rc",
            "3.14.7rcx",
            "3.14.7c1",
            "3.14.7r1",
            "3.13.0t",
            "3.14.7a-1",
            "3.14.7-x86_64",
        ] {
            assert!(
                !PythonManager::is_python_version(malformed),
                "malformed version {malformed:?} must be rejected"
            );
        }
    }

    #[test]
    fn asset_variants_other_than_the_standard_gzip_suffix_are_rejected() {
        let target = "x86_64-unknown-linux-gnu";
        let matching = "cpython-3.14.7+20260825-x86_64-unknown-linux-gnu-install_only.tar.gz";
        assert!(PythonManager::asset_matches_version(
            matching, "3.14.7", target
        ));

        for (name, requested) in [
            (
                "cpython-3.14.7t+20260825-x86_64-unknown-linux-gnu-freethreaded-install_only.tar.gz",
                "3.14.7",
            ),
            (
                "cpython-3.14.7+20260825-x86_64-unknown-linux-gnu-install_only_stripped.tar.gz",
                "3.14.7",
            ),
            (
                "cpython-3.14.7+20260825-x86_64-unknown-linux-gnu-install_only.tar.zst",
                "3.14.7",
            ),
            (
                "cpython-3.14.7+20260825-x86_64-unknown-linux-gnu.tar.gz",
                "3.14.7",
            ),
        ] {
            assert!(
                !PythonManager::asset_matches_version(name, requested, target),
                "variant asset {name:?} must not match"
            );
        }
    }

    fn single_asset_release(tag: &str, prerelease: bool, asset: &str) -> GithubRelease {
        GithubRelease {
            tag_name: tag.to_string(),
            prerelease,
            assets: vec![super::super::common::GithubAsset {
                name: asset.to_string(),
                browser_download_url: None,
                digest: None,
            }],
        }
    }

    fn stable_release(tag: &str, asset: &str) -> GithubRelease {
        single_asset_release(tag, false, asset)
    }

    fn prerelease_release(tag: &str, asset: &str) -> GithubRelease {
        single_asset_release(tag, true, asset)
    }

    #[test]
    fn versions_dedupe_and_sort_newest_first_with_prerelease_precedence() {
        let versions = PythonManager::parse_python_versions(
            &[
                stable_release(
                    "20260825",
                    "cpython-3.14.7+20260825-x86_64-unknown-linux-gnu-install_only.tar.gz",
                ),
                prerelease_release(
                    "20260710",
                    "cpython-3.15.0rc2+20260710-x86_64-unknown-linux-gnu-install_only.tar.gz",
                ),
                stable_release(
                    "20260701",
                    "cpython-3.14.7+20260701-x86_64-unknown-linux-gnu-install_only.tar.gz",
                ),
                stable_release(
                    "20260620",
                    "cpython-3.15.0+20260620-x86_64-unknown-linux-gnu-install_only.tar.gz",
                ),
                prerelease_release(
                    "20260601",
                    "cpython-3.15.0rc1+20260601-x86_64-unknown-linux-gnu-install_only.tar.gz",
                ),
                stable_release(
                    "20260501",
                    "cpython-3.13.9+20260501-x86_64-unknown-linux-gnu-install_only.tar.gz",
                ),
            ],
            "x86_64-unknown-linux-gnu",
        );

        assert_eq!(
            versions,
            vec![
                PythonVersion {
                    version: "3.15.0".to_string(),
                    prerelease: false
                },
                PythonVersion {
                    version: "3.15.0rc2".to_string(),
                    prerelease: true
                },
                PythonVersion {
                    version: "3.15.0rc1".to_string(),
                    prerelease: true
                },
                PythonVersion {
                    version: "3.14.7".to_string(),
                    prerelease: false
                },
                PythonVersion {
                    version: "3.13.9".to_string(),
                    prerelease: false
                },
            ]
        );
    }

    #[test]
    fn duplicate_assets_dedupe_independent_of_the_github_release_flag() {
        for releases in [
            vec![
                prerelease_release(
                    "a",
                    "cpython-3.14.0+20260101-x86_64-unknown-linux-gnu-install_only.tar.gz",
                ),
                stable_release(
                    "b",
                    "cpython-3.14.0+20260201-x86_64-unknown-linux-gnu-install_only.tar.gz",
                ),
            ],
            vec![
                stable_release(
                    "b",
                    "cpython-3.14.0+20260201-x86_64-unknown-linux-gnu-install_only.tar.gz",
                ),
                prerelease_release(
                    "a",
                    "cpython-3.14.0+20260101-x86_64-unknown-linux-gnu-install_only.tar.gz",
                ),
            ],
        ] {
            let versions =
                PythonManager::parse_python_versions(&releases, "x86_64-unknown-linux-gnu");
            assert_eq!(
                versions,
                vec![PythonVersion {
                    version: "3.14.0".to_string(),
                    prerelease: false
                }]
            );
        }
    }

    #[test]
    fn python_version_cmp_orders_prerelease_serials_numerically() {
        use std::cmp::Ordering;
        assert_eq!(
            PythonManager::python_version_cmp("3.15.0rc2", "3.15.0rc10"),
            Ordering::Less
        );
        assert_eq!(
            PythonManager::python_version_cmp("3.15.0", "3.15.0rc2"),
            Ordering::Greater
        );
        assert_eq!(
            PythonManager::python_version_cmp("3.15.0a1", "3.15.0b1"),
            Ordering::Less
        );
        assert_eq!(
            PythonManager::python_version_cmp("3.14.7", "3.15.0rc2"),
            Ordering::Less
        );
    }

    #[test]
    fn stable_only_versions_exclude_prereleases_for_partial_resolution() {
        let available = [
            PythonVersion {
                version: "3.15.0rc2".to_string(),
                prerelease: true,
            },
            PythonVersion {
                version: "3.14.7".to_string(),
                prerelease: false,
            },
            PythonVersion {
                version: "3.14.0".to_string(),
                prerelease: false,
            },
        ];
        let stable: Vec<String> = available
            .iter()
            .filter(|entry| !entry.prerelease)
            .map(|entry| entry.version.clone())
            .collect();
        assert_eq!(
            crate::runtimes::common::resolve_partial_version(&stable, "3.14").as_deref(),
            Some("3.14.7")
        );
        assert_eq!(
            crate::runtimes::common::resolve_partial_version(&stable, "3.15").as_deref(),
            None,
            "a prerelease-only line must not satisfy a partial request"
        );
    }

    #[test]
    fn asset_version_matching_is_component_bounded() {
        let name = "cpython-3.10.21+20260825-x86_64-unknown-linux-gnu-install_only.tar.gz";
        assert!(PythonManager::asset_matches_version(
            name,
            "3.10.21",
            "x86_64-unknown-linux-gnu"
        ));
        assert!(!PythonManager::asset_matches_version(
            name,
            "3.1",
            "x86_64-unknown-linux-gnu"
        ));
        assert!(!PythonManager::asset_matches_version(
            name,
            "3.10",
            "x86_64-unknown-linux-gnu"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn python_manager_activates_vendor_symlink_layout() {
        let temp = tempfile::tempdir().expect("temp dir");
        let version_dir = temp.path().join("3.12.0");
        fs::create_dir_all(version_dir.join("bin")).expect("bin dir");
        fs::write(version_dir.join("bin/python3.12"), b"python").expect("python binary");
        std::os::unix::fs::symlink("python3.12", version_dir.join("bin/python3"))
            .expect("vendor launcher link");
        let manager = PythonManager {
            versions_dir: temp.path().to_path_buf(),
            client: download_client(),
        };

        manager
            .use_version("3.12.0")
            .expect("vendor layout must activate");

        assert_eq!(
            fs::read_link(temp.path().join("current")).expect("current link"),
            version_dir
        );
    }

    #[test]
    fn partial_request_resolves_to_the_newest_matching_python_fixture() {
        let names = ["3.12.0", "3.12.8", "3.11.0"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        assert_eq!(
            crate::runtimes::common::resolve_partial_version(&names, "3.12").as_deref(),
            Some("3.12.8")
        );
        assert_eq!(
            crate::runtimes::common::resolve_partial_version(&names, "3").as_deref(),
            Some("3.12.8")
        );
    }

    #[test]
    fn python_target_uses_host_os_and_arch() {
        let target = python_target().expect("host platform should be supported");
        if std::env::consts::OS == "linux" {
            assert!(target.contains("linux-gnu"));
        } else {
            assert!(!target.contains("linux-gnu"));
        }
    }
}
