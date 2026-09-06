//! Native Java runtime manager - PURE RUST
//!
//! Downloads JDK from Eclipse Adoptium (Temurin).
//!
//! Features:
//! - Official Eclipse Adoptium builds
//! - LTS version detection
//! - `JAVA_HOME` auto-configuration

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use super::common::{
    activate_version, begin_staged_install, complete_staged_install, download_with_progress,
    extract_tar_gz, normalize_version, parse_sha256_digest, print_already_installed,
    print_installed, remove_file_best_effort, validate_download_filename,
};
use crate::{cli::style, core::http::download_client};

const ADOPTIUM_API: &str = "https://api.adoptium.net/v3";

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    link: String,
    name: String,
    checksum: String,
}

/// Java version info
#[derive(Debug, Clone)]
pub(crate) struct JavaVersion {
    pub version: String,
    pub lts: bool,
}

pub(crate) struct JavaManager {
    versions_dir: PathBuf,
    client: &'static reqwest::Client,
}

impl JavaManager {
    pub fn new() -> Self {
        Self {
            versions_dir: super::DATA_DIR.join("versions/java"),
            client: download_client(),
        }
    }

    /// List available Java versions from Adoptium
    pub async fn list_available(&self) -> Result<Vec<JavaVersion>> {
        #[derive(Deserialize)]
        struct AvailableReleases {
            available_lts_releases: Vec<u32>,
            available_releases: Vec<u32>,
        }

        let releases: AvailableReleases = self
            .client
            .get(format!("{ADOPTIUM_API}/info/available_releases"))
            .send()
            .await
            .context("Failed to fetch Java versions from Adoptium")?
            .error_for_status()
            .context("Adoptium version-list request failed")?
            .json()
            .await
            .context("Failed to parse Java version data")?;

        let lts_set: HashSet<u32> = releases.available_lts_releases.into_iter().collect();

        let mut versions: Vec<JavaVersion> = releases
            .available_releases
            .into_iter()
            .map(|v| JavaVersion {
                version: v.to_string(),
                lts: lts_set.contains(&v),
            })
            .collect();

        versions.sort_by_key(|v| std::cmp::Reverse(v.version.parse::<u32>().unwrap_or(0)));

        Ok(versions)
    }

    /// Install Java - PURE RUST, NO SUBPROCESS
    ///
    /// Only Adoptium feature-number requests are accepted; anything else
    /// fails before any network request.
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = java_feature_number(version)?;
        crate::core::security::validate_runtime_version(&version)?;
        let version_dir = self.versions_dir.join(&version);

        if crate::runtimes::common::is_valid_version_dir(&version_dir) {
            print_already_installed("Java", &version);
            return self.use_version(&version);
        }

        println!(
            "{} Installing Java {} (Adoptium)...\n",
            style::runtime("OMG"),
            style::caution(&version)
        );

        let (os, arch) = java_platform()?;

        println!("{} Querying Adoptium API...", style::informative("→"));

        let binaries: Vec<AdoptiumBinary> = self
            .client
            .get(format!(
                "{ADOPTIUM_API}/assets/latest/{version}/hotspot?\
                 architecture={arch}&image_type=jdk&os={os}&vendor=eclipse"
            ))
            .send()
            .await
            .context("Failed to fetch JDK data from Adoptium")?
            .error_for_status()
            .with_context(|| format!("Adoptium has no JDK {version} for {arch}-{os}"))?
            .json()
            .await
            .context("Failed to parse JDK data")?;

        let binary = binaries.first().ok_or_else(|| {
            anyhow::anyhow!("No JDK {version} found for {arch}. Try: omg list java --available")
        })?;

        fs::create_dir_all(&self.versions_dir)?;

        let archive_name = validate_download_filename(&binary.package.name)?;
        println!(
            "{} Downloading {}...",
            style::informative("→"),
            archive_name
        );
        let download_path = self.versions_dir.join(archive_name);
        let checksum = parse_sha256_digest(&binary.package.checksum, "Adoptium")?;
        download_with_progress(self.client, &binary.package.link, &download_path, &checksum)
            .await?;

        println!("{} Extracting (pure Rust)...", style::informative("→"));
        let staging = begin_staged_install(&self.versions_dir)?;
        extract_tar_gz(&download_path, staging.path(), 1).await?;
        complete_staged_install(&staging, &version_dir, &version)?;

        remove_file_best_effort(&download_path, "runtime archive");

        print_installed("Java", &version);
        self.use_version(&version)
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = java_feature_number(version)?;
        let version_dir = self.versions_dir.join(&version);
        activate_version(&self.versions_dir, &version, Path::new("bin/java"))?;

        println!("{} Now using Java {version}", style::positive("✓"));
        println!("  {} {}", style::dim("JAVA_HOME:"), version_dir.display());
        println!(
            "  {} {}",
            style::dim("PATH:"),
            self.versions_dir.join("current/bin").display()
        );

        Ok(())
    }

    /// Remove an installed version. Refuses the active version.
    pub fn uninstall(&self, version: &str) -> Result<()> {
        let version = java_feature_number(version)?;
        super::common::uninstall_version(&self.versions_dir, &version)
    }
}

/// Resolve a Java request to the Adoptium feature number it names.
///
/// Adoptium publishes JDKs by feature: `21` and `21.0` are the same release,
/// while a full update (`21.0.5`), a non-zero minor (`21.1`), a prerelease,
/// or a malformed request names nothing this manager can install.
///
/// Pure and crate-visible so the hook PATH closure can normalize Java pins
/// with the same rule instead of duplicating it.
pub(crate) fn java_feature_number(requested: &str) -> Result<String> {
    let version = normalize_version(requested);
    let is_feature = |component: &str| {
        !component.is_empty() && component.bytes().all(|byte| byte.is_ascii_digit())
    };
    let components: Vec<&str> = version.split('.').collect();
    match components.as_slice() {
        [feature] if is_feature(feature) => Ok(version),
        [feature, "0"] if is_feature(feature) => Ok((*feature).to_owned()),
        _ => Err(anyhow::anyhow!(
            "Invalid Java version {requested:?}: Java installs by feature number (for example 21), not updates such as 21.0.5. Run: omg list java --available"
        )),
    }
}

// Generate common runtime manager methods (list_installed, current_version)
crate::runtimes::common::impl_runtime_common!(JavaManager);

fn java_platform() -> Result<(&'static str, &'static str)> {
    Ok((
        super::common::host_os_tag("Java", "linux", "mac")?,
        super::common::host_arch_tag("Java", "x64", "aarch64")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_manager_new() {
        let mgr = JavaManager::new();
        assert!(mgr.versions_dir.ends_with("java"));
    }

    #[cfg(unix)]
    #[test]
    fn java_manager_normalizes_v_prefixed_versions() {
        let temp = tempfile::tempdir().expect("temp dir");
        let version_dir = temp.path().join("17");
        fs::create_dir_all(version_dir.join("bin")).expect("bin dir");
        fs::write(version_dir.join("bin/java"), b"java").expect("java binary");
        let manager = JavaManager {
            versions_dir: temp.path().to_path_buf(),
            client: download_client(),
        };

        manager.use_version("v17").expect("v prefix must normalize");

        assert_eq!(
            fs::read_link(temp.path().join("current")).expect("current link"),
            version_dir
        );
    }

    #[test]
    fn java_uninstall_uses_the_same_feature_number_as_install() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let version_dir = directory.path().join("21");
        fs::create_dir(&version_dir)?;
        let manager = JavaManager {
            versions_dir: directory.path().to_path_buf(),
            client: download_client(),
        };

        assert!(manager.uninstall("21.0.1").is_err());
        assert!(version_dir.exists());
        manager.uninstall("v21.0")?;
        assert!(!version_dir.exists());
        Ok(())
    }

    #[test]
    fn java_requests_resolve_to_their_adoptium_feature_number() {
        assert_eq!(java_feature_number("21").unwrap(), "21");
        assert_eq!(java_feature_number("v21").unwrap(), "21");
        assert_eq!(java_feature_number("V21").unwrap(), "21");
        assert_eq!(java_feature_number("21.0").unwrap(), "21");
        assert_eq!(java_feature_number("v21.0").unwrap(), "21");
        assert_eq!(java_feature_number("17").unwrap(), "17");
    }

    #[test]
    fn non_feature_java_requests_fail_and_point_to_the_available_list() {
        for request in [
            "21.0.5", "21.0.0", "21.1", "21-ea", "21.0-ea", "", "latest", "21.x", "21.", ".21",
            "2.1.2.1",
        ] {
            let error = java_feature_number(request)
                .err()
                .unwrap_or_else(|| panic!("request {request:?} must fail"));
            let message = error.to_string();
            assert!(
                message.contains("omg list java --available"),
                "error for {request:?} must point to the available list: {message}"
            );
        }
    }

    #[tokio::test]
    async fn install_rejects_non_feature_requests_before_network_access() -> Result<()> {
        let manager = JavaManager::new();
        let error = manager.install("21.0.5").await.unwrap_err();
        assert!(error.to_string().contains("omg list java --available"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn activation_keeps_sibling_executables_in_the_runtime_bin_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let version_dir = temp.path().join("21");
        fs::create_dir_all(version_dir.join("bin")).expect("bin dir");
        fs::write(version_dir.join("bin/java"), b"java").expect("java binary");
        fs::write(version_dir.join("bin/javac"), b"javac").expect("javac binary");
        let manager = JavaManager {
            versions_dir: temp.path().to_path_buf(),
            client: download_client(),
        };

        manager
            .use_version("21")
            .expect("feature request must activate");

        assert_eq!(
            fs::read_link(temp.path().join("current")).expect("current link"),
            version_dir
        );
        assert_eq!(
            fs::read(temp.path().join("current/bin/javac")).expect("sibling stays reachable"),
            b"javac".to_vec()
        );
    }

    #[test]
    fn java_platform_is_host_specific() {
        let (os, _arch) = java_platform().expect("host platform should be supported");
        if std::env::consts::OS == "linux" {
            assert_eq!(os, "linux");
        } else {
            assert_ne!(os, "linux");
        }
    }
}
