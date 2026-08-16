//! Native Rust toolchain manager - PURE RUST, NO RUSTUP
//!
//! Downloads Rust toolchains directly from static.rust-lang.org
//!
//! Features:
//! - Full toolchain management (stable, beta, nightly, dated)
//! - Component installation (rustfmt, clippy, rust-src, etc.)
//! - Cross-compilation target support
//! - Profile-based installation (minimal, default, complete)
//! - rust-toolchain.toml support

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tar::Archive;

use super::common::{
    activate_version, begin_staged_install, complete_staged_install, copy_regular_tree,
    download_with_progress, get_current_version, is_valid_version_dir, list_installed_versions,
    parse_sha256_digest, print_already_installed, print_installed, print_using,
    remove_file_best_effort, replace_staged_install,
};
use crate::core::archive::stripped_archive_path;
use crate::core::http::download_client;

const RUST_DIST_URL: &str = "https://static.rust-lang.org/dist";
const RUST_MANIFEST_PREFIX: &str = "channel-rust";
const RUST_METADATA_FILE: &str = ".omg-toolchain.toml";

/// Rust version info
#[derive(Debug, Clone)]
pub struct RustVersion {
    pub version: String,
    pub channel: String,
}

fn manifest_component_version(manifest: &toml::Value, component: &str) -> Result<String> {
    let value = manifest
        .get("pkg")
        .and_then(|pkg| pkg.get(component))
        .and_then(|pkg| pkg.get("version"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Missing version for component '{component}'"))?;
    Ok(value.split_whitespace().next().unwrap_or(value).to_owned())
}

fn is_date_parts(year: &str, month: &str, day: &str) -> bool {
    year.len() == 4
        && month.len() == 2
        && day.len() == 2
        && year.chars().all(|c| c.is_ascii_digit())
        && month.chars().all(|c| c.is_ascii_digit())
        && day.chars().all(|c| c.is_ascii_digit())
}

#[derive(Debug, Clone)]
pub struct RustToolchainSpec {
    pub channel: String,
    pub date: Option<String>,
    pub host: String,
}

#[derive(Debug, Clone, Default)]
pub struct RustToolchainRequest {
    pub channel: String,
    pub profile: Option<String>,
    pub components: Vec<String>,
    pub targets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RustToolchainStatus {
    pub name: String,
    pub needs_install: bool,
    pub missing_components: Vec<String>,
    pub missing_targets: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RustToolchainFile {
    toolchain: RustToolchainSection,
}

#[derive(Debug, Deserialize)]
pub struct RustToolchainSection {
    channel: String,
    components: Option<Vec<String>>,
    targets: Option<Vec<String>>,
    profile: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct RustToolchainMetadata {
    components: BTreeSet<String>,
    targets: BTreeSet<String>,
}

pub struct RustManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}

impl RustManager {
    pub fn new() -> Self {
        let data_dir = &*super::DATA_DIR;
        let versions_dir = data_dir.join("versions").join("rust");

        Self {
            current_link: versions_dir.join("current"),
            versions_dir,
            client: download_client().clone(),
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.join("bin")
    }

    /// List available Rust versions (stable, beta, nightly + recent releases)
    pub async fn list_available(&self) -> Result<Vec<RustVersion>> {
        let mut versions = Vec::new();

        // Get stable version from manifest
        if let Ok(manifest) = self.fetch_manifest("stable", None).await
            && let Some(version) = manifest_version(&manifest)
        {
            versions.push(RustVersion {
                version,
                channel: "stable".to_string(),
            });
        }

        // Add channel aliases
        versions.push(RustVersion {
            version: "stable".to_string(),
            channel: "stable".to_string(),
        });
        versions.push(RustVersion {
            version: "beta".to_string(),
            channel: "beta".to_string(),
        });
        versions.push(RustVersion {
            version: "nightly".to_string(),
            channel: "nightly".to_string(),
        });

        Ok(versions)
    }

    pub fn list_installed(&self) -> Result<Vec<String>> {
        list_installed_versions(&self.versions_dir)
    }

    #[must_use]
    pub fn current_version(&self) -> Option<String> {
        get_current_version(&self.versions_dir)
    }

    /// Install Rust - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let toolchain = RustToolchainSpec::parse(version)?;
        let version_dir = self.toolchain_dir(&toolchain);

        Self::reject_invalid_toolchain_path(&version_dir)?;
        if is_valid_version_dir(&version_dir) {
            print_already_installed("Rust", &toolchain.name());
            return self.use_version(version);
        }

        let prefix = "OMG".cyan().bold().to_string();
        let toolchain_name = toolchain.name().yellow().to_string();
        tracing::info!("{prefix} Installing Rust {toolchain_name}...\n");

        self.install_with_profile(&toolchain, "default", &[], &[])
            .await?;
        self.use_version(version)?;

        Ok(())
    }

    pub fn toolchain_status(&self, request: &RustToolchainRequest) -> Result<RustToolchainStatus> {
        let toolchain = RustToolchainSpec::parse(&request.channel)?;
        let version_dir = self.toolchain_dir(&toolchain);
        Self::reject_invalid_toolchain_path(&version_dir)?;
        let needs_install = !is_valid_version_dir(&version_dir);
        let missing_components = self.missing_components(&toolchain, &request.components)?;
        let missing_targets = self.missing_targets(&toolchain, &request.targets)?;

        Ok(RustToolchainStatus {
            name: toolchain.name(),
            needs_install,
            missing_components,
            missing_targets,
        })
    }

    pub async fn ensure_toolchain(&self, request: &RustToolchainRequest) -> Result<()> {
        let toolchain = RustToolchainSpec::parse(&request.channel)?;
        let profile = request.profile.as_deref().unwrap_or("default");
        let version_dir = self.toolchain_dir(&toolchain);
        Self::reject_invalid_toolchain_path(&version_dir)?;
        let needs_install = !is_valid_version_dir(&version_dir);
        let needs_components = self.missing_components(&toolchain, &request.components)?;
        let needs_targets = self.missing_targets(&toolchain, &request.targets)?;

        if !needs_install && needs_components.is_empty() && needs_targets.is_empty() {
            return Ok(());
        }

        if needs_install {
            self.install_with_profile(&toolchain, profile, &request.components, &request.targets)
                .await
        } else {
            self.apply_incremental_updates(&toolchain, &needs_components, &needs_targets)
                .await
        }
    }

    pub fn toolchain_dir(&self, toolchain: &RustToolchainSpec) -> PathBuf {
        self.versions_dir.join(toolchain.name())
    }

    fn reject_invalid_toolchain_path(path: &Path) -> Result<()> {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => Ok(()),
            Ok(_) => anyhow::bail!(
                "Refusing to use non-directory Rust toolchain path: {}",
                path.display()
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| {
                format!("Failed to inspect Rust toolchain path: {}", path.display())
            }),
        }
    }

    fn extract_component(
        archive_path: &Path,
        dest_dir: &Path,
        component: &str,
        version: &str,
        target: &str,
    ) -> Result<()> {
        let is_xz = archive_path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext == "xz");

        if is_xz {
            let file = File::open(archive_path)?;
            let mut decompressed = Vec::new();
            lzma_rs::xz_decompress(&mut std::io::BufReader::new(file), &mut decompressed)
                .context("Failed to decompress XZ archive")?;
            let mut archive = Archive::new(decompressed.as_slice());
            Self::extract_component_entries(&mut archive, dest_dir, component, version, target)
        } else {
            let file = File::open(archive_path)?;
            let decoder = GzDecoder::new(file);
            let mut archive = Archive::new(decoder);
            Self::extract_component_entries(&mut archive, dest_dir, component, version, target)
        }
    }

    fn extract_component_entries<R: std::io::Read>(
        archive: &mut Archive<R>,
        dest_dir: &Path,
        component: &str,
        version: &str,
        target: &str,
    ) -> Result<()> {
        let _prefix = format!("{component}-{version}-{target}");

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            let path_str = path.to_string_lossy();

            // Skip manifest and installer files, only extract from the component subdirectory
            if !path_str.contains("/lib/")
                && !path_str.contains("/bin/")
                && !path_str.contains("/share/")
            {
                continue;
            }

            // Skip "component-version-target/component/".
            let Some(stripped) = stripped_archive_path(&path, 2)? else {
                continue;
            };

            let dest_path = dest_dir.join(&stripped);

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let entry_type = entry.header().entry_type();
            if entry_type.is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else if entry_type.is_file() {
                entry.unpack(&dest_path)?;
            } else {
                anyhow::bail!(
                    "Unsupported link or special entry in Rust component archive: {}",
                    path.display()
                );
            }
        }

        Ok(())
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let toolchain = RustToolchainSpec::parse(version)?;
        let toolchain_name = toolchain.name();
        activate_version(&self.versions_dir, &toolchain_name, Path::new("bin/rustc"))?;
        print_using("Rust", &toolchain_name, &self.bin_dir());
        Ok(())
    }

    /// Uninstall a version
    pub fn uninstall(&self, version: &str) -> Result<()> {
        let toolchain = RustToolchainSpec::parse(version)?;
        let version_dir = self.toolchain_dir(&toolchain);

        match fs::symlink_metadata(&version_dir) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => {
                anyhow::bail!(
                    "Refusing to remove non-directory Rust toolchain path: {}",
                    version_dir.display()
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!(
                    "{} Rust {} is not installed",
                    "→".dimmed(),
                    toolchain.name()
                );
                return Ok(());
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to inspect Rust toolchain path: {}",
                        version_dir.display()
                    )
                });
            }
        }

        if let Some(current) = self.current_version()
            && current == toolchain.name()
        {
            // Best-effort: the toolchain directory is removed next. A leftover
            // current symlink is repaired by the next successful use_version.
            remove_file_best_effort(&self.current_link, "Rust current symlink");
        }

        fs::remove_dir_all(&version_dir)?;
        tracing::info!("{} Rust {} uninstalled", "✓".green(), toolchain.name());
        Ok(())
    }
}

impl RustManager {
    pub fn parse_toolchain_file(path: &Path) -> Result<RustToolchainRequest> {
        let is_toml = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "rust-toolchain.toml");

        if is_toml {
            let content = fs::read_to_string(path)?;
            let parsed: RustToolchainFile = toml::from_str(&content)?;
            return Ok(RustToolchainRequest {
                channel: parsed.toolchain.channel,
                profile: parsed.toolchain.profile,
                components: parsed.toolchain.components.unwrap_or_default(),
                targets: parsed.toolchain.targets.unwrap_or_default(),
            });
        }

        let channel = fs::read_to_string(path)?.trim().to_string();
        Ok(RustToolchainRequest {
            channel,
            ..Default::default()
        })
    }

    async fn install_with_profile(
        &self,
        toolchain: &RustToolchainSpec,
        profile: &str,
        components: &[String],
        targets: &[String],
    ) -> Result<()> {
        let version_dir = self.toolchain_dir(toolchain);
        let mut required_components = profile_components(profile)?;
        required_components.extend_from_slice(components);
        required_components.sort_unstable();
        required_components.dedup();

        // First-time installs extract into a same-filesystem staging directory
        // and publish only after every component and the metadata file land.
        let staging = begin_staged_install(&self.versions_dir)?;
        let dest_dir = staging.path();

        for component in &required_components {
            self.install_component(toolchain, dest_dir, component, &toolchain.host)
                .await?;
        }

        for target in targets {
            self.install_component(toolchain, dest_dir, "rust-std", target)
                .await?;
        }

        let mut metadata = Self::read_metadata(dest_dir)?;
        metadata.components.extend(required_components);
        metadata.targets.extend(targets.iter().cloned());
        Self::write_metadata(dest_dir, &metadata)?;
        complete_staged_install(&staging, &version_dir, &toolchain.name())?;

        print_installed("Rust", &toolchain.name());

        Ok(())
    }

    async fn apply_incremental_updates(
        &self,
        toolchain: &RustToolchainSpec,
        components: &[String],
        targets: &[String],
    ) -> Result<()> {
        let version_dir = self.toolchain_dir(toolchain);
        if !is_valid_version_dir(&version_dir) {
            anyhow::bail!(
                "Rust toolchain is not installed as a valid directory: {}",
                version_dir.display()
            );
        }

        let staging = begin_staged_install(&self.versions_dir)?;
        copy_regular_tree(&version_dir, staging.path())?;

        for component in components {
            self.install_component(toolchain, staging.path(), component, &toolchain.host)
                .await?;
        }
        for target in targets {
            self.install_component(toolchain, staging.path(), "rust-std", target)
                .await?;
        }

        let mut metadata = Self::read_metadata(staging.path())?;
        metadata.components.extend(components.iter().cloned());
        metadata.targets.extend(targets.iter().cloned());
        Self::write_metadata(staging.path(), &metadata)?;
        replace_staged_install(&staging, &version_dir, &toolchain.name())
    }

    async fn install_component(
        &self,
        toolchain: &RustToolchainSpec,
        dest_dir: &Path,
        component: &str,
        target: &str,
    ) -> Result<()> {
        tracing::info!("{} Downloading {}...", "→".blue(), component);
        let manifest = self
            .fetch_manifest(&toolchain.channel, toolchain.date.as_deref())
            .await?;
        let component_version = manifest_component_version(&manifest, component)?;
        let url = manifest_component_url(&manifest, component, target)?;
        let filename = url
            .rsplit('/')
            .next()
            .ok_or_else(|| anyhow::anyhow!("Invalid download URL for {component}"))?;
        let checksum = manifest_component_checksum(&manifest, component, target, &url)?;
        let download_path = self.versions_dir.join(filename);

        download_with_progress(&self.client, &url, &download_path, Some(&checksum)).await?;
        tracing::info!("{} Extracting {}...", "→".blue(), component);
        Self::extract_component(
            &download_path,
            dest_dir,
            component,
            &component_version,
            target,
        )?;
        remove_file_best_effort(&download_path, "Rust component archive");
        Ok(())
    }

    fn read_metadata(toolchain_dir: &Path) -> Result<RustToolchainMetadata> {
        let path = toolchain_dir.join(RUST_METADATA_FILE);
        if !path.exists() {
            return Ok(RustToolchainMetadata::default());
        }
        let content = fs::read_to_string(path)?;
        toml::from_str(&content).map_err(Into::into)
    }

    fn write_metadata(toolchain_dir: &Path, metadata: &RustToolchainMetadata) -> Result<()> {
        let path = toolchain_dir.join(RUST_METADATA_FILE);
        let content = toml::to_string_pretty(metadata)?;
        let mut file = tempfile::NamedTempFile::new_in(toolchain_dir).with_context(|| {
            format!(
                "Failed to create Rust toolchain metadata in {}",
                toolchain_dir.display()
            )
        })?;
        file.write_all(content.as_bytes())?;
        file.as_file_mut().sync_all()?;
        file.persist(&path)
            .map_err(|error| error.error)
            .with_context(|| {
                format!(
                    "Failed to persist Rust toolchain metadata at {}",
                    path.display()
                )
            })?;
        Ok(())
    }

    fn missing_components(
        &self,
        toolchain: &RustToolchainSpec,
        requested: &[String],
    ) -> Result<Vec<String>> {
        let metadata = Self::read_metadata(&self.toolchain_dir(toolchain))?;
        Ok(requested
            .iter()
            .filter(|component| !metadata.components.contains(*component))
            .cloned()
            .collect())
    }

    fn missing_targets(
        &self,
        toolchain: &RustToolchainSpec,
        requested: &[String],
    ) -> Result<Vec<String>> {
        let metadata = Self::read_metadata(&self.toolchain_dir(toolchain))?;
        Ok(requested
            .iter()
            .filter(|target| !metadata.targets.contains(*target))
            .cloned()
            .collect())
    }

    async fn fetch_manifest(&self, channel: &str, date: Option<&str>) -> Result<toml::Value> {
        let filename = format!("{RUST_MANIFEST_PREFIX}-{channel}.toml");
        let url = match date {
            Some(d) => format!("{RUST_DIST_URL}/{d}/{filename}"),
            None => format!("{RUST_DIST_URL}/{filename}"),
        };

        let manifest = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch Rust version manifest. Check your internet connection.")?
            .text()
            .await
            .context("Failed to read Rust version manifest")?;

        toml::from_str(&manifest).map_err(Into::into)
    }
}

impl Default for RustManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RustToolchainSpec {
    pub fn parse(input: &str) -> Result<Self> {
        let input = input.trim();
        crate::core::security::validate_runtime_version(input)?;
        let mut segments: Vec<&str> = input.split('-').collect();
        let mut channel = segments.first().copied().unwrap_or(input).to_string();

        // Handle dated beta releases (e.g., "1.82.0-beta.5")
        if channel.chars().next().is_some_and(|c| c.is_ascii_digit())
            && let Some(beta_part) = segments.get(1)
            && beta_part.starts_with("beta")
        {
            channel = format!("{channel}-{beta_part}");
            segments.drain(0..2);
        } else {
            segments.remove(0);
        }

        // Parse date if present (YYYY-MM-DD)
        let date = match (segments.first(), segments.get(1), segments.get(2)) {
            (Some(&y), Some(&m), Some(&d)) if is_date_parts(y, m, d) => {
                segments.drain(0..3);
                Some(format!("{y}-{m}-{d}"))
            }
            _ => None,
        };

        let host = if segments.is_empty() {
            default_host_triple()?
        } else {
            segments.join("-")
        };

        Ok(Self {
            channel,
            date,
            host,
        })
    }

    pub fn name(&self) -> String {
        match &self.date {
            Some(date) => format!("{}-{}-{}", self.channel, date, self.host),
            None => format!("{}-{}", self.channel, self.host),
        }
    }
}

fn default_host_triple() -> Result<String> {
    match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => Ok("x86_64-unknown-linux-gnu".to_string()),
        ("aarch64", "linux") => Ok("aarch64-unknown-linux-gnu".to_string()),
        ("x86_64", "macos") => Ok("x86_64-apple-darwin".to_string()),
        ("aarch64", "macos") => Ok("aarch64-apple-darwin".to_string()),
        ("x86_64", "windows") => Ok("x86_64-pc-windows-msvc".to_string()),
        ("aarch64", "windows") => Ok("aarch64-pc-windows-msvc".to_string()),
        (arch, os) => anyhow::bail!("Unsupported host platform: {arch}-{os}"),
    }
}

fn profile_components(profile: &str) -> Result<Vec<String>> {
    let components = match profile {
        "minimal" => vec!["rustc", "cargo", "rust-std"],
        "default" | "complete" => vec![
            "rustc",
            "cargo",
            "rust-std",
            "rustfmt",
            "clippy",
            "rust-docs",
        ],
        other => anyhow::bail!("Unknown Rust profile: {other}"),
    };
    Ok(components.into_iter().map(String::from).collect())
}

fn manifest_version(manifest: &toml::Value) -> Option<String> {
    manifest
        .get("pkg")
        .and_then(|pkg| pkg.get("rustc"))
        .and_then(|rustc| rustc.get("version"))
        .and_then(|value| value.as_str())
        .map(|value| value.split_whitespace().next().unwrap_or(value).to_string())
}

fn manifest_component_target<'a>(
    manifest: &'a toml::Value,
    component: &str,
    target: &str,
) -> Result<&'a toml::Value> {
    manifest
        .get("pkg")
        .and_then(|pkg| pkg.get(component))
        .and_then(|pkg| pkg.get("target"))
        .and_then(|targets| targets.get(target))
        .ok_or_else(|| anyhow::anyhow!("Target '{target}' not found for component '{component}'"))
}

fn manifest_component_url(manifest: &toml::Value, component: &str, target: &str) -> Result<String> {
    let target_info = manifest_component_target(manifest, component, target)?;
    let url = target_info
        .get("xz_url")
        .and_then(toml::Value::as_str)
        .or_else(|| target_info.get("url").and_then(toml::Value::as_str))
        .ok_or_else(|| anyhow::anyhow!("No download URL for {component} on {target}"))?;

    Ok(url.to_string())
}

fn manifest_component_checksum(
    manifest: &toml::Value,
    component: &str,
    target: &str,
    url: &str,
) -> Result<String> {
    let target_info = manifest_component_target(manifest, component, target)?;
    let digest = target_info
        .get(
            if Path::new(url)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("xz"))
            {
                "xz_hash"
            } else {
                "hash"
            },
        )
        .and_then(toml::Value::as_str)
        .or_else(|| target_info.get("hash").and_then(toml::Value::as_str))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Missing SHA-256 checksum for Rust component {component} target {target}"
            )
        })?;
    parse_sha256_digest(digest, "Rust distribution manifest")
}

#[cfg(test)]
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;
    use std::io::Cursor;
    use tar::{Builder, EntryType, Header};
    use tempfile::TempDir;

    fn component_archive(path: &str, entry_type: EntryType, contents: &[u8]) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        {
            let mut builder = Builder::new(&mut bytes);
            let mut header = Header::new_gnu();
            header.set_entry_type(entry_type);
            header.set_mode(0o755);
            header.set_size(contents.len() as u64);
            header.set_cksum();
            builder.append_data(&mut header, path, contents)?;
            builder.finish()?;
        }
        Ok(bytes)
    }

    #[test]
    fn test_rust_manager_new() {
        let mgr = RustManager::new();
        assert!(mgr.versions_dir.ends_with("rust"));
    }

    #[test]
    fn test_toolchain_spec_parse_stable() {
        let spec = RustToolchainSpec::parse("stable").unwrap();
        assert_eq!(spec.channel, "stable");
        assert!(spec.date.is_none());
    }

    #[test]
    fn test_toolchain_spec_parse_nightly() {
        let spec = RustToolchainSpec::parse("nightly").unwrap();
        assert_eq!(spec.channel, "nightly");
        assert!(spec.date.is_none());
    }

    #[test]
    fn test_toolchain_spec_parse_dated() {
        let spec = RustToolchainSpec::parse("nightly-2024-01-15").unwrap();
        assert_eq!(spec.channel, "nightly");
        assert_eq!(spec.date, Some("2024-01-15".to_string()));
    }

    #[test]
    fn toolchain_specs_reject_unsafe_path_components() {
        for input in ["", ".", "..", "current", "../nightly", "/nightly"] {
            assert!(RustToolchainSpec::parse(input).is_err(), "{input:?}");
        }
    }

    #[test]
    fn test_toolchain_spec_name() {
        let spec = RustToolchainSpec::parse("stable").unwrap();
        let name = spec.name();
        assert!(name.starts_with("stable-"));
        // Platform-agnostic check: should contain any valid OS component
        assert!(name.contains("linux") || name.contains("darwin") || name.contains("windows"));
    }

    #[test]
    fn component_extraction_writes_regular_files_inside_the_destination() -> Result<()> {
        let bytes = component_archive(
            "rustc-1.0.0-target/rustc/bin/rustc",
            EntryType::Regular,
            b"runtime",
        )?;
        let mut archive = Archive::new(Cursor::new(bytes));
        let destination = TempDir::new()?;

        RustManager::extract_component_entries(
            &mut archive,
            destination.path(),
            "rustc",
            "1.0.0",
            "target",
        )?;

        assert_eq!(fs::read(destination.path().join("bin/rustc"))?, b"runtime");
        Ok(())
    }

    #[test]
    fn component_extraction_rejects_links() -> Result<()> {
        let bytes = component_archive(
            "rustc-1.0.0-target/rustc/bin/rustc",
            EntryType::Symlink,
            b"",
        )?;
        let mut archive = Archive::new(Cursor::new(bytes));
        let destination = TempDir::new()?;

        let result = RustManager::extract_component_entries(
            &mut archive,
            destination.path(),
            "rustc",
            "1.0.0",
            "target",
        );

        assert!(result.is_err());
        assert!(!destination.path().join("bin/rustc").exists());
        Ok(())
    }

    #[test]
    fn write_metadata_replaces_existing_file_atomically() -> Result<()> {
        let destination = TempDir::new()?;
        let first = RustToolchainMetadata {
            components: BTreeSet::from(["rustc".to_string()]),
            targets: BTreeSet::new(),
        };
        let second = RustToolchainMetadata {
            components: BTreeSet::from(["rustc".to_string(), "cargo".to_string()]),
            targets: BTreeSet::from(["x86_64-unknown-linux-gnu".to_string()]),
        };

        RustManager::write_metadata(destination.path(), &first)?;
        RustManager::write_metadata(destination.path(), &second)?;

        let loaded = RustManager::read_metadata(destination.path())?;
        assert_eq!(loaded.components, second.components);
        assert_eq!(loaded.targets, second.targets);
        Ok(())
    }

    #[test]
    fn first_install_staging_does_not_publish_until_complete() -> Result<()> {
        let versions = TempDir::new()?;
        let version_dir = versions.path().join("stable-x86_64-unknown-linux-gnu");
        let staging = begin_staged_install(versions.path())?;
        fs::write(staging.path().join("bin-placeholder"), b"partial")?;
        RustManager::write_metadata(
            staging.path(),
            &RustToolchainMetadata {
                components: BTreeSet::from(["rustc".to_string()]),
                targets: BTreeSet::new(),
            },
        )?;

        assert!(!version_dir.exists());
        assert!(list_installed_versions(versions.path())?.is_empty());

        complete_staged_install(&staging, &version_dir, "stable-x86_64-unknown-linux-gnu")?;
        assert!(version_dir.join(RUST_METADATA_FILE).is_file());
        assert_eq!(
            list_installed_versions(versions.path())?,
            vec!["stable-x86_64-unknown-linux-gnu".to_string()]
        );
        Ok(())
    }

    #[test]
    fn incremental_replace_keeps_existing_files_and_new_metadata() -> Result<()> {
        let versions = TempDir::new()?;
        let version_dir = versions.path().join("stable-x86_64-unknown-linux-gnu");
        fs::create_dir_all(version_dir.join("bin"))?;
        fs::write(version_dir.join("bin/rustc"), b"old")?;
        RustManager::write_metadata(
            &version_dir,
            &RustToolchainMetadata {
                components: BTreeSet::from(["rustc".to_string()]),
                targets: BTreeSet::new(),
            },
        )?;

        let staging = begin_staged_install(versions.path())?;
        copy_regular_tree(&version_dir, staging.path())?;
        fs::write(staging.path().join("bin/clippy"), b"new")?;
        let mut metadata = RustManager::read_metadata(staging.path())?;
        metadata.components.insert("clippy".to_string());
        RustManager::write_metadata(staging.path(), &metadata)?;
        replace_staged_install(&staging, &version_dir, "stable-x86_64-unknown-linux-gnu")?;

        assert_eq!(fs::read(version_dir.join("bin/rustc"))?, b"old");
        assert_eq!(fs::read(version_dir.join("bin/clippy"))?, b"new");
        let loaded = RustManager::read_metadata(&version_dir)?;
        assert!(loaded.components.contains("rustc"));
        assert!(loaded.components.contains("clippy"));
        Ok(())
    }

    #[test]
    fn reject_invalid_toolchain_path_blocks_files_and_allows_missing() -> Result<()> {
        let temp = TempDir::new()?;
        let missing = temp.path().join("missing");
        RustManager::reject_invalid_toolchain_path(&missing)?;

        let file_path = temp.path().join("file");
        fs::write(&file_path, b"not a dir")?;
        let error = RustManager::reject_invalid_toolchain_path(&file_path)
            .expect_err("regular files must be rejected");
        assert!(error.to_string().contains("non-directory"));
        Ok(())
    }

    #[test]
    fn test_profile_components() {
        let minimal = profile_components("minimal").unwrap();
        assert!(minimal.contains(&"rustc".to_string()));
        assert!(minimal.contains(&"cargo".to_string()));
        assert!(!minimal.contains(&"clippy".to_string()));

        let default = profile_components("default").unwrap();
        assert!(default.contains(&"clippy".to_string()));
        assert!(default.contains(&"rustfmt".to_string()));
    }

    #[test]
    fn test_default_host_triple() {
        let triple = default_host_triple().unwrap();
        // Platform-agnostic check: should contain any valid OS component
        assert!(
            triple.contains("linux") || triple.contains("darwin") || triple.contains("windows")
        );
    }
}
